/* MinGW-w64 11 lacks the Windows 10+ thread power-throttling declarations
   that ggml uses to opt its worker threads out of EcoQoS on Windows 11. */
#ifdef _WIN32
#include <windows.h>
#ifndef THREAD_POWER_THROTTLING_CURRENT_VERSION
typedef struct _THREAD_POWER_THROTTLING_STATE {
  ULONG Version;
  ULONG ControlMask;
  ULONG StateMask;
} THREAD_POWER_THROTTLING_STATE;
#define THREAD_POWER_THROTTLING_CURRENT_VERSION 1
#define THREAD_POWER_THROTTLING_EXECUTION_SPEED 0x1
#endif
#endif
