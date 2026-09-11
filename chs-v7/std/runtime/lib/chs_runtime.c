#include <limits.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define HT_IMPLEMENTATION
#include "ht.h"

#include "gc/gc.h"

typedef struct {
    char *data;
    size_t len;
} chs_string_t;

typedef struct {
    void *data;
    size_t len;
} chs_slice_t;

extern void chs_main();

chs_slice_t chs_args;

__attribute__((weak)) bool chs_debug_alloc_feature = false;
__attribute__((weak)) bool chs_use_gc_feature = false;

static Ht(uintptr_t, bool) DEBUG_RUNTIME_ALLOCATOR_TABLE = {0};

static void track_allocation(void *ptr) {
    if (chs_debug_alloc_feature && ptr != NULL) {
        *ht_put(&DEBUG_RUNTIME_ALLOCATOR_TABLE, (uintptr_t)ptr) = true;
    }
}

void chs_fatal_error(chs_string_t msg) {
    if (msg.data && msg.len > 0) {
        fprintf(stderr, "%.*s\n", (int)msg.len, msg.data);
    } else {
        fprintf(stderr, "Fatal error!\n");
    }
    abort();
}

void *chs_alloc(size_t size) {
    if (chs_debug_alloc_feature) {
        fprintf(stderr, "chs_alloc: size=%zu\n", size);
    }
    if (size == 0) {
        return NULL;
    }
    void *ptr = chs_use_gc_feature ? gc_malloc(&gc, size) : malloc(size);
    track_allocation(ptr);
    if (chs_debug_alloc_feature && ptr != NULL) {
        fprintf(stderr, "chs_alloc: returning ptr=%p\n", ptr);
    }
    return ptr;
}

void *chs_realloc(void *ptr, size_t size) {
    if (chs_debug_alloc_feature) {
        fprintf(stderr, "chs_realloc: ptr=%p, size=%zu\n", ptr, size);
        if (ptr) {
            if (ht_find(&DEBUG_RUNTIME_ALLOCATOR_TABLE, (uintptr_t)ptr) == NULL) {
                fprintf(stderr,
                        "chs_realloc: ptr=%p was not allocated by the runtime. "
                        "abort()\n",
                        ptr);
                abort();
            }
        }
    }
    void *new_ptr = chs_use_gc_feature ? gc_realloc(&gc, ptr, size) : realloc(ptr, size);
    if (chs_debug_alloc_feature) {
        if (new_ptr && ptr && ptr != new_ptr) {
            ht_find_and_delete(&DEBUG_RUNTIME_ALLOCATOR_TABLE, (uintptr_t)ptr);
        }
        if (new_ptr != NULL) {
            *ht_put(&DEBUG_RUNTIME_ALLOCATOR_TABLE, (uintptr_t)new_ptr) = true;
        }
        fprintf(stderr, "chs_realloc: returning new_ptr=%p\n", new_ptr);
    }

    return new_ptr;
}

void chs_dealloc(void *ptr) {
    if (chs_debug_alloc_feature) {
        if (ptr) {
            if (ht_find(&DEBUG_RUNTIME_ALLOCATOR_TABLE, (uintptr_t)ptr) == NULL) {
                fprintf(stderr,
                        "chs_dealloc: ptr=%p was not allocated by the runtime. "
                        "abort()\n",
                        ptr);
                abort();
            }
            ht_find_and_delete(&DEBUG_RUNTIME_ALLOCATOR_TABLE, (uintptr_t)ptr);
        }
        fprintf(stderr, "chs_dealloc: ptr=%p\n", ptr);
    }
    if (chs_use_gc_feature) {
        gc_free(&gc, ptr);
    } else {
        free(ptr);
    }
}

void chs__oob_check(chs_string_t m, size_t idx, size_t len) {
    bool is_oob = idx >= len;
    if (is_oob) {
        if (m.data && m.len > 0) {
            fprintf(stderr, "%.*s: index %zu, length %zu\n", (int)m.len, m.data, idx, len);
        } else {
            fprintf(stderr, "Index out of bounds: index %zu, length %zu\n", idx, len);
        }
        abort();
    }
}

__attribute__((used)) void chs__init_runtime(int argc, char **argv) {
    if (chs_debug_alloc_feature) {
        fprintf(stderr, "CHS runtime initialized\n");
    }
    if (chs_use_gc_feature) {
        gc_start(&gc, &argc);
    }
    chs_string_t *elements = chs_alloc(argc * sizeof(chs_string_t));
    if (chs_use_gc_feature)
        gc_make_static(&gc, elements);
    for (int i = 0; i < argc; i++) {
        elements[i].data = argv[i];
        elements[i].len = strlen(argv[i]);
    }
    chs_args.data = (char *)elements;
    chs_args.len = argc;
}

__attribute__((used)) void chs__deinit_runtime() {
    chs_dealloc(chs_args.data);
    if (chs_use_gc_feature) {
        gc_stop(&gc);
    } else if (chs_debug_alloc_feature) {
        bool leaks_found = false;
        ht_foreach(value, &DEBUG_RUNTIME_ALLOCATOR_TABLE) {
            void *ptr = (void *)ht_key(&DEBUG_RUNTIME_ALLOCATOR_TABLE, value);
            fprintf(stderr,
                    "MEMORY LEAK DETECTED: ptr=%p was never deallocated. "
                    "Cleaning up...\n",
                    ptr);
            free(ptr);
            leaks_found = true;
        }

        if (!leaks_found) {
            fprintf(stderr, "CHS runtime: No memory leaks detected.\n");
        }

        ht_free(&DEBUG_RUNTIME_ALLOCATOR_TABLE);
        fprintf(stderr, "CHS runtime deinitialized\n");
    }
}

__attribute__((used)) int chs_start_main(int argc, char **argv) {
    chs__init_runtime(argc, argv);
    chs_main();
    chs__deinit_runtime();
    exit(0);
}

void _start(void) {
    __asm__ volatile("xor %rbp, %rbp\n\t"
                     "mov (%rsp), %rsi\n\t"                        /* 2nd arg: argc = *rsp */
                     "lea 8(%rsp), %rdx\n\t"                       /* 3rd arg: argv = rsp + 8 */
                     "push %rsp\n\t"                               /* 7th arg: stack_end */
                     "push $0\n\t"                                 /* dummy push for 16-byte stack alignment */
                     "xor %r9d, %r9d\n\t"                          /* 6th arg: rtld_fini = NULL */
                     "xor %r8d, %r8d\n\t"                          /* 5th arg: fini = NULL */
                     "xor %ecx, %ecx\n\t"                          /* 4th arg: init = NULL */
                     "mov chs_start_main@GOTPCREL(%rip), %rdi\n\t" /* 1st arg: main =
                                                                      chs_start_main */
                     "call __libc_start_main@PLT\n\t");
}
