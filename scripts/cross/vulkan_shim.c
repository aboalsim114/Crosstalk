/*
 * ggml's Vulkan backend takes a single function from the Vulkan loader,
 * vkGetInstanceProcAddr, and resolves everything else through it. Linking
 * vulkan-1.dll directly would stop Crosstalk from starting on a PC without a
 * Vulkan driver; this shim (built as libvulkan.a by cross-build-windows-gnu.sh)
 * loads it at run time from System32 instead. Without it, the only function
 * it returns is a vkEnumerateInstanceVersion that fails: ggml then reports no
 * GPU and Whisper runs on the CPU.
 */
#include <string.h>
#include <windows.h>

typedef void (*vk_void_fn)(void);
typedef vk_void_fn (*vk_get_proc_fn)(void *instance, const char *name);

static vk_get_proc_fn real_get_proc;
static volatile LONG tried;

/* VK_ERROR_INITIALIZATION_FAILED: Vulkan-Hpp throws, ggml catches it. */
static int vk_unavailable(unsigned int *version) {
    (void)version;
    return -3;
}

vk_void_fn vkGetInstanceProcAddr(void *instance, const char *name) {
    if (InterlockedCompareExchange(&tried, 1, 0) == 0) {
        HMODULE dll = LoadLibraryExW(L"vulkan-1.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
        if (dll) {
            real_get_proc = (vk_get_proc_fn)(void *)GetProcAddress(dll, "vkGetInstanceProcAddr");
        }
        MemoryBarrier();
    }
    if (real_get_proc) {
        return real_get_proc(instance, name);
    }
    if (name && strcmp(name, "vkEnumerateInstanceVersion") == 0) {
        return (vk_void_fn)vk_unavailable;
    }
    return NULL;
}
