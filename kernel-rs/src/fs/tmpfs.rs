//! tmpfs: small hierarchical in-RAM filesystem.
//!
//! Fixed node table (dirs and files) + a bump-allocated byte pool for file
//! contents. Writes that extend a file re-allocate the whole region at the
//! pool top (never freed) -- simple and correct enough for a teaching kernel.
//! Paths are absolute from "/". Components "." and ".." are honoured.

use core::cell::UnsafeCell;

pub const VFS_NAME_MAX: usize = 24;

pub const VN_FREE: u8 = 0;
pub const VN_FILE: u8 = 1;
pub const VN_DIR: u8 = 2;

const TN_MAX: usize = 192; // nodes incl. root
const TN_POOL: usize = 1024 * 1024; // 1 MiB total file data

#[derive(Clone, Copy)]
struct TNode {
    name: [u8; VFS_NAME_MAX],
    typ: u8, // VN_*
    parent: u16,
    off: u32,  // byte offset into pool (files)
    size: u32,
}

impl TNode {
    const fn free() -> Self {
        TNode {
            name: [0; VFS_NAME_MAX],
            typ: VN_FREE,
            parent: 0,
            off: 0,
            size: 0,
        }
    }
}

struct NodesCell(UnsafeCell<[TNode; TN_MAX]>);
unsafe impl Sync for NodesCell {}
struct PoolCell(UnsafeCell<[u8; TN_POOL]>);
unsafe impl Sync for PoolCell {}

static NODES: NodesCell = NodesCell(UnsafeCell::new([TNode::free(); TN_MAX]));
static POOL: PoolCell = PoolCell(UnsafeCell::new([0; TN_POOL]));
static mut POOL_USED: u32 = 0;

fn nodes() -> &'static mut [TNode; TN_MAX] {
    unsafe { &mut *NODES.0.get() }
}

fn pool() -> &'static mut [u8; TN_POOL] {
    unsafe { &mut *POOL.0.get() }
}

fn name_bytes(n: &[u8; VFS_NAME_MAX]) -> &[u8] {
    let end = n.iter().position(|&b| b == 0).unwrap_or(VFS_NAME_MAX);
    &n[..end]
}

/* ------------------------------------------------------------------ */
/* path resolution                                                     */

fn child_of(parent: i32, name: &[u8]) -> i32 {
    let nodes = nodes();
    for i in 1..TN_MAX {
        if nodes[i].typ == VN_FREE || nodes[i].parent as i32 != parent {
            continue;
        }
        if name_bytes(&nodes[i].name) == name {
            return i as i32;
        }
    }
    -1
}

/// Resolve absolute path -> node id, or -1. Path must start with '/'.
fn resolve(path: &str) -> i32 {
    let nodes = nodes();
    let bytes = path.as_bytes();
    if bytes.is_empty() || bytes[0] != b'/' {
        return -1;
    }
    let mut cur: i32 = 0; // root
    let mut p = 1usize;
    while p < bytes.len() {
        while p < bytes.len() && bytes[p] == b'/' {
            p += 1;
        }
        if p >= bytes.len() {
            break;
        }
        let mut e = p;
        while e < bytes.len() && bytes[e] != b'/' {
            e += 1;
        }
        let comp = &bytes[p..e];
        if comp == b"." {
            // stay
        } else if comp == b".." {
            if cur != 0 {
                cur = nodes[cur as usize].parent as i32;
            }
        } else {
            if comp.len() >= VFS_NAME_MAX {
                return -1;
            }
            if nodes[cur as usize].typ != VN_DIR {
                return -1;
            }
            cur = child_of(cur, comp);
            if cur < 0 {
                return -1;
            }
        }
        p = e;
    }
    cur
}

/// Resolve all but the last component; returns `(parent, leaf, leaf_len)`
/// where `leaf_len` indexes into `leaf`, or None on failure.
fn resolve_parent(path: &str) -> Option<(i32, [u8; VFS_NAME_MAX], usize)> {
    let nodes = nodes();
    let bytes = path.as_bytes();
    if bytes.is_empty() || bytes[0] != b'/' {
        return None;
    }
    let mut cur: i32 = 0;
    let mut p = 1usize;
    loop {
        while p < bytes.len() && bytes[p] == b'/' {
            p += 1;
        }
        if p >= bytes.len() {
            return None; // no leaf component
        }
        let mut e = p;
        while e < bytes.len() && bytes[e] != b'/' {
            e += 1;
        }
        let comp = &bytes[p..e];
        let mut e2 = e;
        while e2 < bytes.len() && bytes[e2] == b'/' {
            e2 += 1;
        }
        if e2 >= bytes.len() {
            // last component
            if comp.is_empty() || comp.len() >= VFS_NAME_MAX {
                return None;
            }
            if comp == b"." || comp == b".." {
                return None;
            }
            let mut leaf = [0u8; VFS_NAME_MAX];
            leaf[..comp.len()].copy_from_slice(comp);
            return Some((cur, leaf, comp.len()));
        }
        if comp == b"." {
            p = e2;
            continue;
        }
        if comp == b".." {
            if cur != 0 {
                cur = nodes[cur as usize].parent as i32;
            }
            p = e2;
            continue;
        }
        if comp.len() >= VFS_NAME_MAX {
            return None;
        }
        if nodes[cur as usize].typ != VN_DIR {
            return None;
        }
        cur = child_of(cur, comp);
        if cur < 0 {
            return None;
        }
        p = e2;
    }
}

fn alloc_node(parent: i32, name: &[u8], typ: u8) -> i32 {
    let nodes = nodes();
    for i in 1..TN_MAX {
        if nodes[i].typ != VN_FREE {
            continue;
        }
        nodes[i].typ = if typ == VN_FILE { VN_FILE } else { VN_DIR };
        nodes[i].parent = parent as u16;
        nodes[i].off = unsafe { POOL_USED };
        nodes[i].size = 0;
        let n = name.len().min(VFS_NAME_MAX - 1);
        nodes[i].name[..n].copy_from_slice(&name[..n]);
        nodes[i].name[n..].fill(0);
        return i as i32;
    }
    -1
}

/* ------------------------------------------------------------------ */
/* public API                                                          */

pub fn init() {
    let nodes = nodes();
    *nodes = [TNode::free(); TN_MAX];
    unsafe {
        POOL_USED = 0;
    }
    nodes[0].typ = VN_DIR; // "/"
    nodes[0].parent = 0;
    crate::klog_info!(
        "vfs: tmpfs tree mounted at / ({} nodes, {} KiB pool)",
        TN_MAX,
        TN_POOL / 1024
    );
}

pub fn path_type(path: &str) -> u8 {
    let id = resolve(path);
    if id < 0 {
        0
    } else {
        nodes()[id as usize].typ
    }
}

pub fn path_size(path: &str) -> i64 {
    let id = resolve(path);
    if id < 0 || nodes()[id as usize].typ != VN_FILE {
        -1
    } else {
        nodes()[id as usize].size as i64
    }
}

pub fn mkdir(path: &str) -> i32 {
    let Some((parent, leaf, leaf_len)) = resolve_parent(path) else {
        return -1;
    };
    let leaf_name = &leaf[..leaf_len];
    if child_of(parent, leaf_name) >= 0 {
        return -1; // exists
    }
    if alloc_node(parent, leaf_name, VN_DIR) < 0 {
        -1
    } else {
        0
    }
}

/// Create (or truncate to) a file with initial contents. Node id or -1.
pub fn create(path: &str, data: &[u8]) -> i32 {
    let Some((parent, leaf, leaf_len)) = resolve_parent(path) else {
        return -1;
    };
    let leaf_name = &leaf[..leaf_len];
    let mut id = child_of(parent, leaf_name);
    if id < 0 {
        id = alloc_node(parent, leaf_name, VN_FILE);
        if id < 0 {
            return -1;
        }
    } else if nodes()[id as usize].typ != VN_FILE {
        return -1;
    }
    let size = data.len();
    if unsafe { POOL_USED as usize } + size > TN_POOL {
        return -1;
    }
    let off = unsafe { POOL_USED };
    pool()[off as usize..off as usize + size].copy_from_slice(data);
    nodes()[id as usize].off = off;
    nodes()[id as usize].size = size as u32;
    unsafe {
        POOL_USED += size as u32;
    }
    id
}

pub fn unlink(path: &str) -> i32 {
    let id = resolve(path);
    if id <= 0 || nodes()[id as usize].typ != VN_FILE {
        return -1;
    }
    nodes()[id as usize].typ = VN_FREE; // pool bytes leak: by design
    0
}

pub fn rename(oldp: &str, newp: &str) -> i32 {
    let id = resolve(oldp);
    if id <= 0 {
        return -1;
    }
    let Some((parent, leaf, leaf_len)) = resolve_parent(newp) else {
        return -1;
    };
    let leaf_name = &leaf[..leaf_len];
    if child_of(parent, leaf_name) >= 0 {
        return -1;
    }
    let nodes = nodes();
    nodes[id as usize].parent = parent as u16;
    let n = leaf_name.len().min(VFS_NAME_MAX - 1);
    nodes[id as usize].name[..n].copy_from_slice(&leaf_name[..n]);
    nodes[id as usize].name[n..].fill(0);
    0
}

pub fn read_path(path: &str, off: u64, buf: &mut [u8]) -> i64 {
    node_read(resolve(path), off, buf)
}

pub fn write_path(path: &str, off: u64, buf: &[u8]) -> i64 {
    node_write(resolve(path), off, buf)
}

pub fn lookup(path: &str) -> i32 {
    resolve(path)
}

pub fn node_type(id: i32) -> u8 {
    if id < 0 || id as usize >= TN_MAX {
        0
    } else {
        nodes()[id as usize].typ
    }
}

pub fn node_size(id: i32) -> u64 {
    if id < 0 || id as usize >= TN_MAX {
        0
    } else {
        nodes()[id as usize].size as u64
    }
}

/// Raw file bytes for `id`, or None. The slice aliases the pool.
pub fn node_data(id: i32) -> Option<&'static [u8]> {
    if id <= 0 || id as usize >= TN_MAX || nodes()[id as usize].typ != VN_FILE {
        return None;
    }
    let nodes = nodes();
    let off = nodes[id as usize].off as usize;
    let size = nodes[id as usize].size as usize;
    Some(&pool()[off..off + size])
}

pub fn node_read(id: i32, off: u64, buf: &mut [u8]) -> i64 {
    let nodes = nodes();
    if id <= 0 || id as usize >= TN_MAX || nodes[id as usize].typ != VN_FILE {
        return -1;
    }
    let n = &nodes[id as usize];
    if off > n.size as u64 {
        return 0;
    }
    let size = n.size as usize;
    let off = off as usize;
    let mut len = buf.len();
    if off + len > size {
        len = size - off;
    }
    buf[..len].copy_from_slice(&pool()[n.off as usize + off..n.off as usize + off + len]);
    len as i64
}

pub fn node_write(id: i32, off: u64, buf: &[u8]) -> i64 {
    let nodes = nodes();
    if id <= 0 || id as usize >= TN_MAX || nodes[id as usize].typ != VN_FILE {
        return -1;
    }
    if off > nodes[id as usize].size as u64 {
        return -1; // no sparse files
    }
    let need = off + buf.len() as u64;
    if need > nodes[id as usize].size as u64 {
        if unsafe { POOL_USED } as u64 + need > TN_POOL as u64 {
            return -1;
        }
        // relocate the file to fresh pool space grown to `need`
        let newoff = unsafe { POOL_USED };
        let old_off = nodes[id as usize].off as usize;
        let old_size = nodes[id as usize].size as usize;
        let pool = pool();
        pool.copy_within(old_off..old_off + old_size, newoff as usize);
        nodes[id as usize].off = newoff;
        nodes[id as usize].size = need as u32;
        unsafe {
            POOL_USED += need as u32;
        }
    }
    let base = nodes[id as usize].off as usize + off as usize;
    pool()[base..base + buf.len()].copy_from_slice(buf);
    buf.len() as i64
}

/// Directory iteration: fills `out` and returns true while entries remain.
pub struct DirEnt {
    pub name: [u8; VFS_NAME_MAX],
    pub size: u64,
    pub typ: u32, // VN_FILE / VN_DIR
}

pub fn list(dir: &str, index: usize) -> Option<DirEnt> {
    let pid = resolve(dir);
    if pid < 0 || nodes()[pid as usize].typ != VN_DIR {
        return None;
    }
    let nodes = nodes();
    let mut seen = 0usize;
    for i in 1..TN_MAX {
        if nodes[i].typ == VN_FREE || nodes[i].parent as i32 != pid {
            continue;
        }
        if seen != index {
            seen += 1;
            continue;
        }
        let mut name = [0u8; VFS_NAME_MAX];
        name.copy_from_slice(&nodes[i].name);
        return Some(DirEnt {
            name,
            size: nodes[i].size as u64,
            typ: nodes[i].typ as u32,
        });
    }
    None
}
