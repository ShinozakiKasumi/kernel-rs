/* tmpfs: flat root directory of fixed-size in-RAM files.
 * Data lives in a static BSS pool -- plenty for a teaching kernel. */
#include "vfs.h"
#include "kprintf.h"
#include "lib/string.h"

#define TMPFS_MAX_FILES 16
#define TMPFS_POOL (512 * 1024)         /* 512 KiB total file data */

struct tfile {
    char     name[VFS_NAME_MAX];
    uint64_t size;
    uint32_t data_off;                  /* byte offset into pool */
    bool     used;
};

static struct tfile files[TMPFS_MAX_FILES];
static char   pool[TMPFS_POOL];
static uint32_t pool_used;

static struct tfile *find(const char *path) {
    if (path[0] == '/') path++;        /* flat namespace, ignore root slash */
    for (int i = 0; i < TMPFS_MAX_FILES; i++)
        if (files[i].used && !strcmp(files[i].name, path))
            return &files[i];
    return NULL;
}

void vfs_init(void) {
    memset(files, 0, sizeof files);
    pool_used = 0;
    KLOG_INFO("vfs: tmpfs mounted at / (%d file slots, %d KiB pool)",
              TMPFS_MAX_FILES, TMPFS_POOL / 1024);
}

int vfs_create(const char *path, const void *data, uint64_t size) {
    struct tfile *f = find(path);
    if (!f) {
        for (int i = 0; i < TMPFS_MAX_FILES; i++)
            if (!files[i].used) { f = &files[i]; break; }
        if (!f) return -1;
        if (path[0] == '/') path++;
        strncpy(f->name, path, VFS_NAME_MAX - 1);
        f->data_off = pool_used;
        f->used = true;
    }
    if (f->data_off + size > TMPFS_POOL) return -1;
    memcpy(pool + f->data_off, data, size);
    f->size = size;
    pool_used = f->data_off + (uint32_t)size;
    return 0;
}

int64_t vfs_read(const char *path, uint64_t off, void *buf, uint64_t len) {
    struct tfile *f = find(path);
    if (!f || off > f->size) return -1;
    if (off + len > f->size) len = f->size - off;
    memcpy(buf, pool + f->data_off + off, len);
    return (int64_t)len;
}

int64_t vfs_size(const char *path) {
    struct tfile *f = find(path);
    return f ? (int64_t)f->size : -1;
}

bool vfs_list(unsigned index, struct dirent *out) {
    unsigned seen = 0;
    for (int i = 0; i < TMPFS_MAX_FILES; i++) {
        if (!files[i].used) continue;
        if (seen++ == index) {
            strncpy(out->name, files[i].name, VFS_NAME_MAX - 1);
            out->name[VFS_NAME_MAX - 1] = 0;
            out->size = files[i].size;
            out->is_dir = false;
            return true;
        }
    }
    return false;
}
