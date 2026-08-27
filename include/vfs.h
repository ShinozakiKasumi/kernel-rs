#ifndef VFS_H
#define VFS_H

#include <stdint.h>
#include <stdbool.h>

#define VFS_NAME_MAX 32

struct dirent {
    char     name[VFS_NAME_MAX];
    uint64_t size;
    bool     is_dir;
};

/* Mount tmpfs as the root namespace. */
void vfs_init(void);

/* Create (or overwrite) a regular file with initial contents. */
int  vfs_create(const char *path, const void *data, uint64_t size);

/* Read `len` bytes from `off`; returns bytes read or -1 on error. */
int64_t vfs_read(const char *path, uint64_t off, void *buf, uint64_t len);

/* File size or -1 if missing. */
int64_t vfs_size(const char *path);

/* Directory iteration: index 0..; returns true while entries remain. */
bool vfs_list(unsigned index, struct dirent *out);

#endif
