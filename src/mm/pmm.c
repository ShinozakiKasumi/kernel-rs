/* Physical page allocator: static bitmap over the Limine memory map.
 *
 * bit = 1 -> free, 0 -> used/unusable. Only Limine "usable" entries become
 * free; kernel image, modules, reclaimable and firmware regions stay used.
 */
#include "mm/pmm.h"
#include "kprintf.h"
#include "lib/string.h"

#define PMM_MAX_PHYS (512ULL << 20)                 /* cap: 512 MiB */
#define BITMAP_PAGES (PMM_MAX_PHYS / PAGE_SIZE)     /* 131072 pages */
#define BITMAP_WORDS (BITMAP_PAGES / 64)            /* 2048 words = 16KiB */

static uint64_t bitmap[BITMAP_WORDS];  /* starts zeroed: everything used */

static size_t total_pages, free_pages;

/* --- Limine requests ------------------------------------------------------- */

struct limine_memmap_entry {
    uint64_t base, length, type;
};
#define LIMINE_MEMMAP_USABLE 0

struct limine_memmap_response {
    uint64_t revision, entry_count;
    struct limine_memmap_entry **entries;
};
struct limine_hhdm_response { uint64_t revision, offset; };

struct limine_memmap_request {
    uint64_t id[4];
    uint64_t revision;
    struct limine_memmap_response *response;
};
struct limine_hhdm_request {
    uint64_t id[4];
    uint64_t revision;
    struct limine_hhdm_response *response;
};

__attribute__((used, section(".limine_requests")))
static volatile struct limine_memmap_request memmap_req = {
    .id = { 0xc7b1dd30df4c8b88ULL, 0x0a82e883a194f07bULL,
            0x67cf3d9d378a806fULL, 0xe304acdfc50c3c62ULL },
};
__attribute__((used, section(".limine_requests")))
static volatile struct limine_hhdm_request hhdm_req = {
    .id = { 0xc7b1dd30df4c8b88ULL, 0x0a82e883a194f07bULL,
            0x48dcf1cb8ad2b852ULL, 0x63984e959a98244bULL },
};

uint64_t hhdm_offset;

/* --- bitmap ops ------------------------------------------------------------- */

static inline void bm_set(size_t i)   { bitmap[i >> 6] |=  (1ULL << (i & 63)); }
static inline void bm_clear(size_t i) { bitmap[i >> 6] &= ~(1ULL << (i & 63)); }
static inline int  bm_test(size_t i)  { return (bitmap[i >> 6] >> (i & 63)) & 1; }

void pmm_init(void) {
    if (!memmap_req.response || !hhdm_req.response) {
        KLOG_ERR("pmm: missing limine memmap/hhdm response");
        for (;;) __asm__ volatile ("hlt");
    }
    hhdm_offset = hhdm_req.response->offset;
    KLOG_INFO("pmm: hhdm offset=%p", (void *)hhdm_offset);

    struct limine_memmap_response *mm = memmap_req.response;
    uint64_t top = 0;

    for (uint64_t i = 0; i < mm->entry_count; i++) {
        struct limine_memmap_entry *e = mm->entries[i];
        if (e->type != LIMINE_MEMMAP_USABLE)
            continue;

        uint64_t base = (e->base + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);
        uint64_t end  = (e->base + e->length) & ~(PAGE_SIZE - 1);
        if (end > PMM_MAX_PHYS) end = PMM_MAX_PHYS;
        if (base >= end) continue;

        for (uint64_t p = base; p < end; p += PAGE_SIZE) {
            bm_set(p / PAGE_SIZE);
            free_pages++;
            total_pages++;
        }
        if (end > top) top = end;
    }
    KLOG_INFO("pmm: %lu pages free (%lu MiB), top=%#lx",
              free_pages, free_pages * PAGE_SIZE >> 20, top);
}

uintptr_t pmm_alloc_page(void) {
    for (size_t w = 0; w < BITMAP_WORDS; w++) {
        if (!bitmap[w]) continue;
        int bit = __builtin_ctzll(bitmap[w]);
        size_t idx = (w << 6) + (size_t)bit;
        bm_clear(idx);
        free_pages--;
        return (uintptr_t)idx * PAGE_SIZE;
    }
    return 0;
}

uintptr_t pmm_alloc_pages(size_t count, size_t align) {
    if (align < PAGE_SIZE) align = PAGE_SIZE;
    size_t align_pages = align / PAGE_SIZE;

    for (size_t i = 0; i + count <= BITMAP_PAGES; i++) {
        if (i % align_pages) { i -= i % align_pages; continue; }
        size_t j = 0;
        for (; j < count; j++)
            if (!bm_test(i + j)) break;
        if (j == count) {
            for (j = 0; j < count; j++) bm_clear(i + j);
            free_pages -= count;
            return (uintptr_t)i * PAGE_SIZE;
        }
        i += j;   /* skip past the used page (loop adds 1) */
    }
    return 0;
}

void pmm_free_page(uintptr_t pa) {
    size_t idx = pa / PAGE_SIZE;
    if (pa % PAGE_SIZE || idx >= BITMAP_PAGES || bm_test(idx)) {
        KLOG_ERR("pmm: bad free of %p", (void *)pa);
        return;
    }
    bm_set(idx);
    free_pages++;
}

size_t pmm_free_count(void)  { return free_pages; }
size_t pmm_total_count(void) { return total_pages; }

/* --- test ------------------------------------------------------------------- */

void pmm_selftest(void) {
    size_t before = pmm_free_count();

    uintptr_t a = pmm_alloc_page();
    uintptr_t b = pmm_alloc_page();
    if (!a || !b || a == b || (a % PAGE_SIZE) || (b % PAGE_SIZE))
        KLOG_ERR("pmm test FAIL: a=%#lx b=%#lx", a, b);
    if (pmm_free_count() != before - 2)
        KLOG_ERR("pmm test FAIL: count after alloc = %lu", pmm_free_count());

    pmm_free_page(a);
    if (pmm_free_count() != before - 1)
        KLOG_ERR("pmm test FAIL: count after one free");

    uintptr_t run = pmm_alloc_pages(4, PAGE_SIZE);
    if (!run || run % PAGE_SIZE)
        KLOG_ERR("pmm test FAIL: 4-page run");
    pmm_free_page(b);
    KLOG_INFO("pmm: selftest ok (a=%#lx b=%#lx run=%#lx)", a, b, run);
}
