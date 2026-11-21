//! Ramfs: the initial Aster filesystem.
//!
//! This is deliberately simple and fully in memory: a flat inode table with
//! parent/child links, absolute path resolution, and byte-granular reads and
//! writes. Every operation is testable and deterministic. The on-disk AFS
//! (journal, checksums, snapshots) is a later milestone; ramfs is the VFS
//! contract that AFS must implement.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::storage::fat32;
use crate::sync::IrqLock;

/// Top byte of an inode selects the mounted filesystem. 0 is ramfs, 1 is the
/// persistent FAT32 home. Native inode numbers are preserved below it.
const FAT_TAG: u64 = fat32::MOUNT_TAG;

fn fat_relative(path: &str) -> Option<&str> {
    let (kind, prefix) = crate::mount::resolve(path)?;
    if kind != crate::mount::FsKind::Fat32 {
        return None;
    }
    if prefix == 0 {
        Some(path)
    } else if prefix >= path.len() {
        Some("/")
    } else {
        Some(&path[prefix..])
    }
}

fn fat_native(inode: u64) -> Option<u64> {
    if inode & FAT_TAG != 0 {
        Some(inode & !FAT_TAG)
    } else {
        None
    }
}

fn tagged(native: u64) -> u64 {
    FAT_TAG | native
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    File,
    Dir,
}

#[derive(Clone)]
pub struct Inode {
    pub id: u64,
    pub name: String,
    pub kind: NodeKind,
    pub parent: u64,
    pub children: Vec<u64>,
    pub data: Vec<u8>,
    pub mode: u32,
    pub modified_ms: u64,
}

struct Fs {
    nodes: Vec<Inode>,
    next_id: u64,
}

unsafe impl Send for Fs {}

static FS: IrqLock<Fs> = IrqLock::new(Fs {
    nodes: Vec::new(),
    next_id: 1,
});

pub const MAX_FILE_SIZE: usize = 4 * 1024 * 1024;
pub const MAX_FDS: usize = 24;

#[derive(Clone, Copy)]
pub struct OpenFile {
    pub inode: u64,
    pub offset: u64,
    pub flags: u32,
    pub dir_index: usize,
}

impl Fs {
    fn node(&self, id: u64) -> Option<&Inode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    fn node_mut(&mut self, id: u64) -> Option<&mut Inode> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    fn root(&self) -> u64 {
        1
    }

    fn create(
        &mut self,
        parent: u64,
        name: &str,
        kind: NodeKind,
        data: Vec<u8>,
    ) -> Result<u64, i64> {
        if name.is_empty() || name.len() > 63 || name.contains('/') {
            return Err(crate::syscall::errno::EINVAL);
        }
        let parent_node = self.node(parent).ok_or(crate::syscall::errno::ENOENT)?;
        if parent_node.kind != NodeKind::Dir {
            return Err(crate::syscall::errno::ENOTDIR);
        }
        if parent_node
            .children
            .iter()
            .any(|&child| self.node(child).map(|n| n.name == name).unwrap_or(false))
        {
            return Err(crate::syscall::errno::EEXIST);
        }
        let id = self.next_id;
        self.next_id += 1;
        let now = crate::time::uptime_ms();
        self.nodes.push(Inode {
            id,
            name: name.to_string(),
            kind,
            parent,
            children: Vec::new(),
            data,
            mode: if kind == NodeKind::Dir { 0o755 } else { 0o644 },
            modified_ms: now,
        });
        if let Some(parent_node) = self.node_mut(parent) {
            parent_node.children.push(id);
        }
        Ok(id)
    }

    fn resolve(&self, path: &str) -> Result<u64, i64> {
        if !path.starts_with('/') {
            return Err(crate::syscall::errno::EINVAL);
        }
        let mut current = self.root();
        for component in path.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            if component == ".." {
                current = self.node(current).map(|n| n.parent).unwrap_or(current);
                continue;
            }
            let node = self.node(current).ok_or(crate::syscall::errno::ENOENT)?;
            if node.kind != NodeKind::Dir {
                return Err(crate::syscall::errno::ENOTDIR);
            }
            let next = node
                .children
                .iter()
                .filter_map(|&id| self.node(id))
                .find(|child| child.name == component)
                .map(|child| child.id);
            current = next.ok_or(crate::syscall::errno::ENOENT)?;
        }
        Ok(current)
    }

    fn resolve_parent<'a>(&self, path: &'a str) -> Result<(u64, &'a str), i64> {
        let trimmed = path.trim_end_matches('/');
        let index = trimmed.rfind('/').ok_or(crate::syscall::errno::EINVAL)?;
        let parent_path = if index == 0 { "/" } else { &trimmed[..index] };
        let name = &trimmed[index + 1..];
        if name.is_empty() || name == "." || name == ".." {
            return Err(crate::syscall::errno::EINVAL);
        }
        Ok((self.resolve(parent_path)?, name))
    }

    fn unlink_recursive(&mut self, id: u64) {
        if id == self.root() {
            return;
        }
        let children: Vec<u64> = self
            .node(id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child in children {
            self.unlink_recursive(child);
        }
        let parent = self.node(id).map(|n| n.parent);
        if let Some(parent) = parent {
            if let Some(parent_node) = self.node_mut(parent) {
                parent_node.children.retain(|&child| child != id);
            }
        }
        self.nodes.retain(|node| node.id != id);
    }
}

pub fn init() {
    let mut fs = FS.lock();
    if !fs.nodes.is_empty() {
        return;
    }
    fs.nodes.push(Inode {
        id: 1,
        name: String::from("/"),
        kind: NodeKind::Dir,
        parent: 1,
        children: Vec::new(),
        data: Vec::new(),
        mode: 0o755,
        modified_ms: 0,
    });
    fs.next_id = 2;

    let mut dirs = alloc::vec![
        "/home",
        "/home/user",
        "/etc",
        "/system",
        "/var",
        "/var/log",
        "/apps",
    ];
    if !fat32::mounted() {
        dirs.push("/home/user/Documents");
        dirs.push("/home/user/notes");
    }
    for dir in dirs {
        let (parent, name) = fs.resolve_parent(dir).expect("seed directory");
        fs.create(parent, name, NodeKind::Dir, Vec::new())
            .expect("seed directory");
    }

    let mut files: Vec<(&str, &str)> = alloc::vec![
        (
            "/README.md",
            "# Aster OS\n\nAn independent x86_64 operating system.\n\nThis build boots via UEFI, runs a higher-half kernel with preemptive\nthreads, ring-3 user space, PS/2 input and a software compositor.\n\nUser files live on a persistent FAT32 volume when a data disk is present;\nsystem files are still served from the in-memory ramfs.\n\nTry the Terminal: type `help`.\n",
        ),
        (
            "/etc/motd",
            "Welcome to Aster OS. A new generation of personal computing.\n",
        ),
        (
            "/system/version",
            "Aster OS 0.5.0 (graphics and motion)\nkernel: aster 0.5.0\nabi: 1\n",
        ),
        (
            "/var/log/boot.log",
            "Aster OS boot log\nSee the serial console for the full kernel log.\n",
        ),
    ];
    if !fat32::mounted() {
        files.push((
            "/home/user/welcome.txt",
            "Hello.\n\nThis is your home directory. Documents, notes and settings live here.\nUse the Files app to browse, or the Terminal to explore the system.\n",
        ));
        files.push((
            "/home/user/notes/ideas.txt",
            "Ideas\n-----\n- Native compositor with real window management\n- Persistent AFS with checksums\n- More applications built on the shared UI toolkit\n",
        ));
        files.push((
            "/home/user/settings.conf",
            "theme=light\naccent=blue\nanimations=1.0\nwallpaper=0\nreduce_motion=0\nbrightness=1.0\n",
        ));
    }

    for (path, content) in files {
        let (parent, name) = fs.resolve_parent(path).expect("seed file");
        fs.create(parent, name, NodeKind::File, content.as_bytes().to_vec())
            .expect("seed file");
    }
    crate::info!(
        "ramfs: {} inodes, {} bytes of file data",
        fs.nodes.len(),
        fs.nodes.iter().map(|n| n.data.len()).sum::<usize>()
    );
}

fn errno_value(code: i64) -> i64 {
    code
}

fn validate_open_flags(flags: u32) -> Result<(), i64> {
    const KNOWN: u32 = aster_abi::open_flags::READ
        | aster_abi::open_flags::WRITE
        | aster_abi::open_flags::CREATE
        | aster_abi::open_flags::TRUNC
        | aster_abi::open_flags::DIRECTORY;
    if flags & !KNOWN != 0
        || flags & (aster_abi::open_flags::READ | aster_abi::open_flags::WRITE) == 0
    {
        return Err(crate::syscall::errno::EINVAL);
    }
    if flags & aster_abi::open_flags::TRUNC != 0 && flags & aster_abi::open_flags::WRITE == 0 {
        return Err(crate::syscall::errno::EINVAL);
    }
    Ok(())
}

pub fn open(path: &str, flags: u32) -> Result<u64, i64> {
    validate_open_flags(flags)?;
    if let Some(relative) = fat_relative(path) {
        return fat32::open(relative, flags).map(tagged);
    }
    let mut fs = FS.lock();
    match fs.resolve(path) {
        Ok(id) => {
            let node = fs.node(id).ok_or(crate::syscall::errno::ENOENT)?;
            if flags & aster_abi::open_flags::DIRECTORY != 0 && node.kind != NodeKind::Dir {
                return Err(crate::syscall::errno::ENOTDIR);
            }
            if flags & aster_abi::open_flags::TRUNC != 0 && node.kind == NodeKind::File {
                let node = fs.node_mut(id).unwrap();
                node.data.clear();
                node.modified_ms = crate::time::uptime_ms();
            }
            Ok(id)
        }
        Err(code) if flags & aster_abi::open_flags::CREATE != 0 => {
            let (parent, name) = fs.resolve_parent(path)?;
            fs.create(parent, name, NodeKind::File, Vec::new())
        }
        Err(code) => Err(errno_value(code)),
    }
}

pub fn node_kind(id: u64) -> Option<NodeKind> {
    if let Some(native) = fat_native(id) {
        return fat32::stat(native).ok().map(|stat| {
            if stat.is_dir != 0 {
                NodeKind::Dir
            } else {
                NodeKind::File
            }
        });
    }
    FS.lock().node(id).map(|node| node.kind)
}

pub fn read(id: u64, offset: u64, length: usize) -> Result<Vec<u8>, i64> {
    if let Some(native) = fat_native(id) {
        return fat32::read(native, offset, length);
    }
    let fs = FS.lock();
    let node = fs.node(id).ok_or(crate::syscall::errno::ENOENT)?;
    if node.kind == NodeKind::Dir {
        return Err(crate::syscall::errno::EISDIR);
    }
    let start = offset as usize;
    if start >= node.data.len() {
        return Ok(Vec::new());
    }
    let end = (start + length).min(node.data.len());
    Ok(node.data[start..end].to_vec())
}

pub fn write(id: u64, offset: u64, data: &[u8]) -> Result<usize, i64> {
    if let Some(native) = fat_native(id) {
        return fat32::write(native, offset, data);
    }
    let mut fs = FS.lock();
    let node = fs.node_mut(id).ok_or(crate::syscall::errno::ENOENT)?;
    if node.kind == NodeKind::Dir {
        return Err(crate::syscall::errno::EISDIR);
    }
    let start = offset as usize;
    let end = start + data.len();
    if end > MAX_FILE_SIZE {
        return Err(crate::syscall::errno::ENOSPC);
    }
    if start > node.data.len() {
        node.data.resize(start, 0);
    }
    if end > node.data.len() {
        node.data.resize(end, 0);
    }
    node.data[start..end].copy_from_slice(data);
    node.modified_ms = crate::time::uptime_ms();
    Ok(data.len())
}

pub fn truncate(id: u64) -> Result<(), i64> {
    if let Some(native) = fat_native(id) {
        return fat32::truncate(native);
    }
    let mut fs = FS.lock();
    let node = fs.node_mut(id).ok_or(crate::syscall::errno::ENOENT)?;
    if node.kind != NodeKind::File {
        return Err(crate::syscall::errno::EISDIR);
    }
    node.data.clear();
    Ok(())
}

pub fn size(id: u64) -> Result<u64, i64> {
    if let Some(native) = fat_native(id) {
        return fat32::size(native);
    }
    let fs = FS.lock();
    Ok(fs.node(id).ok_or(crate::syscall::errno::ENOENT)?.data.len() as u64)
}

pub fn stat_path(path: &str) -> Result<aster_abi::FileStat, i64> {
    if let Some(relative) = fat_relative(path) {
        let native = fat32::lookup(relative)?;
        let mut stat = fat32::stat(native)?;
        stat.inode = tagged(native);
        return Ok(stat);
    }
    let fs = FS.lock();
    let id = fs.resolve(path)?;
    let node = fs.node(id).ok_or(crate::syscall::errno::ENOENT)?;
    Ok(aster_abi::FileStat {
        size: node.data.len() as u64,
        is_dir: if node.kind == NodeKind::Dir { 1 } else { 0 },
        mode: node.mode,
        inode: node.id,
    })
}

pub fn stat_inode(id: u64) -> Result<aster_abi::FileStat, i64> {
    if let Some(native) = fat_native(id) {
        let mut stat = fat32::stat(native)?;
        stat.inode = id;
        return Ok(stat);
    }
    let fs = FS.lock();
    let node = fs.node(id).ok_or(crate::syscall::errno::ENOENT)?;
    Ok(aster_abi::FileStat {
        size: node.data.len() as u64,
        is_dir: if node.kind == NodeKind::Dir { 1 } else { 0 },
        mode: node.mode,
        inode: node.id,
    })
}

pub struct DirItem {
    pub inode: u64,
    pub kind: NodeKind,
    pub name: String,
    pub size: u64,
}

pub fn list(id: u64, offset: usize, maximum: usize) -> Result<Vec<DirItem>, i64> {
    if let Some(native) = fat_native(id) {
        let items = fat32::list(native, offset, maximum)?;
        return Ok(items
            .into_iter()
            .map(|item| DirItem {
                inode: tagged(item.inode),
                kind: match item.kind {
                    fat32::NodeKind::Dir => NodeKind::Dir,
                    fat32::NodeKind::File => NodeKind::File,
                },
                name: item.name,
                size: item.size,
            })
            .collect());
    }
    let fs = FS.lock();
    let node = fs.node(id).ok_or(crate::syscall::errno::ENOENT)?;
    if node.kind != NodeKind::Dir {
        return Err(crate::syscall::errno::ENOTDIR);
    }
    let mut items: Vec<DirItem> = Vec::new();
    for child_id in node.children.iter().skip(offset).take(maximum) {
        if let Some(child) = fs.node(*child_id) {
            items.push(DirItem {
                inode: child.id,
                kind: child.kind,
                name: child.name.clone(),
                size: child.data.len() as u64,
            });
        }
    }
    Ok(items)
}

pub fn mkdir(path: &str) -> Result<u64, i64> {
    if let Some(relative) = fat_relative(path) {
        return fat32::mkdir(relative).map(tagged);
    }
    let mut fs = FS.lock();
    if let Ok(existing) = fs.resolve(path) {
        let _ = existing;
        return Err(crate::syscall::errno::EEXIST);
    }
    let (parent, name) = fs.resolve_parent(path)?;
    fs.create(parent, name, NodeKind::Dir, Vec::new())
}

pub fn unlink(path: &str) -> Result<(), i64> {
    if let Some(relative) = fat_relative(path) {
        return fat32::unlink(relative);
    }
    let mut fs = FS.lock();
    let id = fs.resolve(path)?;
    if id == fs.root() {
        return Err(crate::syscall::errno::EPERM);
    }
    fs.unlink_recursive(id);
    Ok(())
}

pub fn rename(old_path: &str, new_path: &str) -> Result<(), i64> {
    let old_is_fat = fat_relative(old_path).is_some();
    let new_is_fat = fat_relative(new_path).is_some();
    if old_is_fat != new_is_fat {
        return Err(crate::syscall::errno::EXDEV);
    }
    if old_is_fat {
        let old_relative = fat_relative(old_path).unwrap();
        let new_relative = fat_relative(new_path).unwrap();
        return fat32::rename(old_relative, new_relative);
    }

    let mut fs = FS.lock();
    let id = fs.resolve(old_path)?;
    if id == fs.root() {
        return Err(crate::syscall::errno::EPERM);
    }
    let (old_parent, _) = fs.resolve_parent(old_path)?;
    let (new_parent, new_name) = fs.resolve_parent(new_path)?;
    if old_parent != new_parent {
        // Cross-directory move is not atomic on ramfs yet; callers copy and
        // delete instead. Keeping this explicit avoids half-renamed trees.
        return Err(crate::syscall::errno::EINVAL);
    }
    let duplicate = fs
        .node(new_parent)
        .map(|parent| {
            parent.children.iter().any(|&child| {
                child != id
                    && fs
                        .node(child)
                        .map(|node| node.name == new_name)
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if duplicate {
        return Err(crate::syscall::errno::EEXIST);
    }
    let node = fs.node_mut(id).ok_or(crate::syscall::errno::ENOENT)?;
    node.name = new_name.to_string();
    node.modified_ms = crate::time::uptime_ms();
    Ok(())
}

pub fn sync() -> i64 {
    crate::storage::sync()
}

pub fn read_whole(path: &str) -> Option<Vec<u8>> {
    if let Some(relative) = fat_relative(path) {
        let native = fat32::lookup(relative).ok()?;
        let size = fat32::size(native).ok()?;
        if size > 256 * 1024 {
            return None;
        }
        let mut data = Vec::with_capacity(size as usize);
        let mut offset = 0u64;
        while offset < size {
            let chunk = fat32::read(native, offset, 64 * 1024).ok()?;
            if chunk.is_empty() {
                break;
            }
            offset += chunk.len() as u64;
            data.extend_from_slice(&chunk);
        }
        return Some(data);
    }
    let mut fs = FS.lock();
    let id = fs.resolve(path).ok()?;
    let node = fs.node(id)?;
    if node.kind != NodeKind::File {
        return None;
    }
    Some(node.data.clone())
}

pub fn write_whole(path: &str, data: &[u8]) -> Result<(), i64> {
    if let Some(relative) = fat_relative(path) {
        let inode = fat32::open(
            relative,
            aster_abi::open_flags::WRITE
                | aster_abi::open_flags::CREATE
                | aster_abi::open_flags::TRUNC,
        )?;
        let mut offset = 0usize;
        while offset < data.len() {
            let end = (offset + 8192).min(data.len());
            let written = fat32::write(inode, offset as u64, &data[offset..end])?;
            if written == 0 {
                return Err(crate::syscall::errno::EIO);
            }
            offset += written;
        }
        return Ok(());
    }
    let mut fs = FS.lock();
    let id = match fs.resolve(path) {
        Ok(id) => id,
        Err(_) => {
            let (parent, name) = fs.resolve_parent(path)?;
            fs.create(parent, name, NodeKind::File, Vec::new())?
        }
    };
    let node = fs.node_mut(id).ok_or(crate::syscall::errno::ENOENT)?;
    if node.kind != NodeKind::File {
        return Err(crate::syscall::errno::EISDIR);
    }
    if data.len() > MAX_FILE_SIZE {
        return Err(crate::syscall::errno::ENOSPC);
    }
    node.data.clear();
    node.data.extend_from_slice(data);
    node.modified_ms = crate::time::uptime_ms();
    Ok(())
}

pub fn inode_count() -> usize {
    FS.lock().nodes.len()
}

/// Self-test: create, write, read, list, rename-by-copy, unlink.
pub fn self_test() -> bool {
    if mkdir("/tmp_test").is_err() {
        return false;
    }
    if write_whole("/tmp_test/a.txt", b"hello aster").is_err() {
        return false;
    }
    if read_whole("/tmp_test/a.txt").as_deref() != Some(b"hello aster".as_ref()) {
        return false;
    }
    let id = match open(
        "/tmp_test",
        aster_abi::open_flags::READ | aster_abi::open_flags::DIRECTORY,
    ) {
        Ok(id) => id,
        Err(_) => return false,
    };
    let items = match list(id, 0, 16) {
        Ok(items) => items,
        Err(_) => return false,
    };
    if !items
        .iter()
        .any(|item| item.name == "a.txt" && item.size == 11)
    {
        return false;
    }
    if unlink("/tmp_test").is_err() {
        return false;
    }
    read_whole("/tmp_test/a.txt").is_none()
}
