#include <CoreAudio/CoreAudio.h>
#include <CoreAudio/AudioServerPlugIn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>

#define MAX_CHANNELS 16
#define MAX_PID_MAPPINGS 64
#define BUFFER_FRAMES 1024
#define SAMPLE_RATE 48000.0
#define DEVICE_UID "com.sendinbeats.audiodriver"
#define DEVICE_NAME "Sendin Beats Virtual Audio"

typedef struct {
    pid_t pid;
    int channel;
    int active;
} PIDMapping;

static PIDMapping gMappings[MAX_PID_MAPPINGS];
static int gMappingCount = 0;
static pthread_mutex_t gMappingMutex = PTHREAD_MUTEX_INITIALIZER;

// Per-channel audio buffers
static float gChannelBuffers[MAX_CHANNELS][BUFFER_FRAMES];
static pthread_mutex_t gBufferMutex = PTHREAD_MUTEX_INITIALIZER;

// Device ID
static AudioObjectID gDeviceID = 1000;
static AudioObjectID gInputStreamID = 2000;
static AudioObjectID gOutputStreamID = 3000;
static AudioServerPlugInHostRef gHost = NULL;

// Forward declarations
static OSStatus PlugIn_QueryInterface(void* inDriver, REFIID inUUID, LPVOID* outInterface);
static ULONG PlugIn_AddRef(void* inDriver);
static ULONG PlugIn_Release(void* inDriver);
static OSStatus PlugIn_Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost);
static OSStatus PlugIn_CreateDevice(AudioServerPlugInDriverRef inDriver, CFDictionaryRef inDescription, const AudioServerPlugInClientInfo* inClientInfo, AudioObjectID* outDeviceObjectID);
static OSStatus PlugIn_DestroyDevice(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID);
static OSStatus PlugIn_AddDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo);
static OSStatus PlugIn_RemoveDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo);
static OSStatus PlugIn_PerformDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo);
static OSStatus PlugIn_AbortDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo);
static Boolean PlugIn_HasProperty(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress);
static OSStatus PlugIn_IsPropertySettable(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, Boolean* outIsSettable);
static OSStatus PlugIn_GetPropertyDataSize(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32* outDataSize);
static OSStatus PlugIn_GetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, UInt32* outDataSize, void* outData);
static OSStatus PlugIn_SetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, const void* inData);
static OSStatus PlugIn_StartIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID);
static OSStatus PlugIn_StopIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID);
static OSStatus PlugIn_GetZeroTimeStamp(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, Float64* outSampleTime, UInt64* outHostTime, UInt64* outSeed);
static OSStatus PlugIn_WillDoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, Boolean* outWillDo, Boolean* outWillDoInPlace);
static OSStatus PlugIn_BeginIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo);
static OSStatus PlugIn_DoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, AudioObjectID inStreamObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo, void* ioMainBuffer, void* ioSecondaryBuffer);
static OSStatus PlugIn_EndIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo);

// PID-to-channel mapping functions
void MapPIDToChannel(pid_t pid, int channel) {
    pthread_mutex_lock(&gMappingMutex);

    // Check if PID already exists
    for (int i = 0; i < gMappingCount; i++) {
        if (gMappings[i].pid == pid) {
            gMappings[i].channel = channel;
            gMappings[i].active = 1;
            pthread_mutex_unlock(&gMappingMutex);
            printf("[Driver] Updated PID %d -> channel %d\n", pid, channel);
            return;
        }
    }

    // Add new mapping
    if (gMappingCount < MAX_PID_MAPPINGS) {
        gMappings[gMappingCount].pid = pid;
        gMappings[gMappingCount].channel = channel;
        gMappings[gMappingCount].active = 1;
        gMappingCount++;
        printf("[Driver] Added PID %d -> channel %d\n", pid, channel);
    }

    pthread_mutex_unlock(&gMappingMutex);
}

void UnmapPID(pid_t pid) {
    pthread_mutex_lock(&gMappingMutex);

    for (int i = 0; i < gMappingCount; i++) {
        if (gMappings[i].pid == pid) {
            gMappings[i].active = 0;
            printf("[Driver] Unmapped PID %d\n", pid);
            break;
        }
    }

    pthread_mutex_unlock(&gMappingMutex);
}

void RouteAudioToChannel(pid_t pid, float* buffer, int frames) {
    if (frames > BUFFER_FRAMES) frames = BUFFER_FRAMES;

    pthread_mutex_lock(&gMappingMutex);

    int ch = -1;
    for (int i = 0; i < gMappingCount; i++) {
        if (gMappings[i].pid == pid && gMappings[i].active) {
            ch = gMappings[i].channel;
            break;
        }
    }

    pthread_mutex_unlock(&gMappingMutex);

    if (ch >= 0 && ch < MAX_CHANNELS) {
        pthread_mutex_lock(&gBufferMutex);
        memcpy(gChannelBuffers[ch], buffer, sizeof(float) * frames);
        pthread_mutex_unlock(&gBufferMutex);
    }
}

// Plugin interface table
static AudioServerPlugInDriverInterface gAudioServerPlugInDriverInterface = {
    NULL,
    PlugIn_QueryInterface,
    PlugIn_AddRef,
    PlugIn_Release,
    PlugIn_Initialize,
    PlugIn_CreateDevice,
    PlugIn_DestroyDevice,
    PlugIn_AddDeviceClient,
    PlugIn_RemoveDeviceClient,
    PlugIn_PerformDeviceConfigurationChange,
    PlugIn_AbortDeviceConfigurationChange,
    PlugIn_HasProperty,
    PlugIn_IsPropertySettable,
    PlugIn_GetPropertyDataSize,
    PlugIn_GetPropertyData,
    PlugIn_SetPropertyData,
    PlugIn_StartIO,
    PlugIn_StopIO,
    PlugIn_GetZeroTimeStamp,
    PlugIn_WillDoIOOperation,
    PlugIn_BeginIOOperation,
    PlugIn_DoIOOperation,
    PlugIn_EndIOOperation
};

static AudioServerPlugInDriverInterface* gAudioServerPlugInDriverInterfacePtr = &gAudioServerPlugInDriverInterface;
static AudioServerPlugInDriverRef gAudioServerPlugInDriverRef = &gAudioServerPlugInDriverInterfacePtr;

// Plugin entry point
void* AudioDriverPlugInOpen(CFAllocatorRef inAllocator, CFUUIDRef inRequestedTypeUUID) {
    if (!CFEqual(inRequestedTypeUUID, kAudioServerPlugInTypeUUID)) {
        return NULL;
    }

    memset(gMappings, 0, sizeof(gMappings));
    memset(gChannelBuffers, 0, sizeof(gChannelBuffers));

    printf("[Driver] Plugin opened\n");
    return gAudioServerPlugInDriverRef;
}

// Interface implementations
static OSStatus PlugIn_QueryInterface(void* inDriver, REFIID inUUID, LPVOID* outInterface) {
    return kAudioHardwareUnsupportedOperationError;
}

static ULONG PlugIn_AddRef(void* inDriver) {
    return 1;
}

static ULONG PlugIn_Release(void* inDriver) {
    return 1;
}

static OSStatus PlugIn_Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost) {
    gHost = inHost;
    printf("[Driver] Initialized\n");
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_CreateDevice(AudioServerPlugInDriverRef inDriver, CFDictionaryRef inDescription, const AudioServerPlugInClientInfo* inClientInfo, AudioObjectID* outDeviceObjectID) {
    if (!outDeviceObjectID) return kAudioHardwareBadObjectError;

    *outDeviceObjectID = gDeviceID;
    printf("[Driver] Created device ID=%u\n", gDeviceID);
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_DestroyDevice(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID) {
    printf("[Driver] Destroyed device %u\n", inDeviceObjectID);
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_AddDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo) {
    if (inClientInfo) {
        printf("[Driver] Added client PID=%d\n", inClientInfo->mClientID);
    }
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_RemoveDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo) {
    if (inClientInfo) {
        printf("[Driver] Removed client PID=%d\n", inClientInfo->mClientID);
        UnmapPID(inClientInfo->mClientID);
    }
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_PerformDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo) {
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_AbortDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo) {
    return kAudioHardwareNoError;
}

static Boolean PlugIn_HasProperty(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress) {
    return false;
}

static OSStatus PlugIn_IsPropertySettable(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, Boolean* outIsSettable) {
    if (outIsSettable) *outIsSettable = false;
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_GetPropertyDataSize(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32* outDataSize) {
    if (outDataSize) *outDataSize = 0;
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_GetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, UInt32* outDataSize, void* outData) {
    if (outDataSize) *outDataSize = 0;
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_SetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, const void* inData) {
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_StartIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID) {
    printf("[Driver] StartIO device=%u client=%u\n", inDeviceObjectID, inClientID);
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_StopIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID) {
    printf("[Driver] StopIO device=%u client=%u\n", inDeviceObjectID, inClientID);
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_GetZeroTimeStamp(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, Float64* outSampleTime, UInt64* outHostTime, UInt64* outSeed) {
    if (outSampleTime) *outSampleTime = 0.0;
    if (outHostTime) *outHostTime = 0;
    if (outSeed) *outSeed = 0;
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_WillDoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, Boolean* outWillDo, Boolean* outWillDoInPlace) {
    if (outWillDo) *outWillDo = true;
    if (outWillDoInPlace) *outWillDoInPlace = true;
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_BeginIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo) {
    return kAudioHardwareNoError;
}

static OSStatus PlugIn_DoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, AudioObjectID inStreamObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo, void* ioMainBuffer, void* ioSecondaryBuffer) {
    if (!ioMainBuffer) return kAudioHardwareNoError;

    // Copy channel buffers to output
    pthread_mutex_lock(&gBufferMutex);

    float* outputBuffer = (float*)ioMainBuffer;
    int framesToCopy = inIOBufferFrameSize < BUFFER_FRAMES ? inIOBufferFrameSize : BUFFER_FRAMES;

    // Mix all active channels
    memset(outputBuffer, 0, sizeof(float) * framesToCopy);

    for (int ch = 0; ch < MAX_CHANNELS; ch++) {
        for (int i = 0; i < framesToCopy; i++) {
            outputBuffer[i] += gChannelBuffers[ch][i];
        }
    }

    pthread_mutex_unlock(&gBufferMutex);

    return kAudioHardwareNoError;
}

static OSStatus PlugIn_EndIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo) {
    return kAudioHardwareNoError;
}
