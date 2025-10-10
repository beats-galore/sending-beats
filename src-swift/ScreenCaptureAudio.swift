import Foundation
import ScreenCaptureKit
import AVFoundation

// MARK: - C-compatible callback types for Rust FFI

/// Audio sample callback: (context_ptr, samples_ptr, sample_count, channels, sample_rate)
public typealias AudioSampleCallback = @convention(c) (
    UnsafeMutableRawPointer?,  // context (Rust side)
    UnsafePointer<Float>?,      // PCM samples (interleaved)
    Int32,                      // sample_count
    Int32,                      // channels
    Float64                     // sample_rate
) -> Void

/// Error callback: (context_ptr, error_message)
public typealias ErrorCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafePointer<CChar>?
) -> Void

// MARK: - Application Info

@objc public class SCAppInfo: NSObject {
    public let pid: Int32
    public let bundleIdentifier: String
    public let applicationName: String

    public init(pid: Int32, bundleIdentifier: String, applicationName: String) {
        self.pid = pid
        self.bundleIdentifier = bundleIdentifier
        self.applicationName = applicationName
    }
}

// MARK: - Stream Output Delegate

class ScreenCaptureAudioOutput: NSObject, SCStreamOutput {
    private let audioCallback: AudioSampleCallback
    private let errorCallback: ErrorCallback
    private let context: UnsafeMutableRawPointer?

    init(audioCallback: @escaping AudioSampleCallback,
         errorCallback: @escaping ErrorCallback,
         context: UnsafeMutableRawPointer?) {
        self.audioCallback = audioCallback
        self.errorCallback = errorCallback
        self.context = context
        super.init()
    }

    private var callbackCount = 0
    private var lastLogTime = Date()
    private var lastCallbackTime: Date?

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio else { return }

        callbackCount += 1
        let now = Date()

        // VERIFY: Measure callback timing
        if let lastTime = lastCallbackTime {
            let callbackInterval = now.timeIntervalSince(lastTime) * 1000.0 // milliseconds
            if callbackCount <= 10 {
                print("🕐 CALLBACK_TIMING: Callback #\(callbackCount) - interval: \(String(format: "%.2f", callbackInterval))ms since last")
            }
        }
        lastCallbackTime = now

        if callbackCount % 1000 == 0 {
            let elapsed = now.timeIntervalSince(lastLogTime)
            print("🎵 SCREEN_CAPTURE_KIT_SWIFT: Received \(callbackCount) audio callbacks (\(String(format: "%.1f", elapsed))s since last log)")
            lastLogTime = now
        }

        // Extract audio buffer from CMSampleBuffer
        guard let blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) else {
            sendError("Failed to get data buffer from sample")
            return
        }

        var length: Int = 0
        var dataPointer: UnsafeMutablePointer<Int8>?

        let status = CMBlockBufferGetDataPointer(
            blockBuffer,
            atOffset: 0,
            lengthAtOffsetOut: nil,
            totalLengthOut: &length,
            dataPointerOut: &dataPointer
        )

        guard status == kCMBlockBufferNoErr, let dataPointer = dataPointer else {
            sendError("Failed to get data pointer from block buffer")
            return
        }

        // Get audio format description
        guard let formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer) else {
            sendError("Failed to get format description")
            return
        }

        let audioStreamBasicDescription = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription)
        guard let asbd = audioStreamBasicDescription?.pointee else {
            sendError("Failed to get audio stream basic description")
            return
        }

        let channels = Int32(asbd.mChannelsPerFrame)
        let sampleRate = Float64(asbd.mSampleRate)

        // Log first callback with format details
        if callbackCount == 1 {
            let formatFlags = asbd.mFormatFlags
            let isFloat = (formatFlags & kAudioFormatFlagIsFloat) != 0
            let bitsPerChannel = asbd.mBitsPerChannel
            let bytesPerFrame = asbd.mBytesPerFrame
            let totalFloat32Values = length / MemoryLayout<Float>.size

            // Check if audio is interleaved or planar
            let isNonInterleaved = (formatFlags & kAudioFormatFlagIsNonInterleaved) != 0
            let isPacked = (formatFlags & kAudioFormatFlagIsPacked) != 0

            print("🎵 SCREEN_CAPTURE_KIT_SWIFT: First audio callback")
            print("   Format: \(isFloat ? "Float\(bitsPerChannel)" : "Int\(bitsPerChannel)"), \(channels) channels, \(sampleRate)Hz")
            print("   Format flags: isPacked=\(isPacked), isNonInterleaved=\(isNonInterleaved)")
            print("   bytesPerFrame from CoreAudio: \(bytesPerFrame)")
            print("   Buffer size: \(length) bytes")
            print("   Total Float32 values in buffer: \(totalFloat32Values)")

            if isNonInterleaved {
                print("   ⚠️ AUDIO IS NON-INTERLEAVED (PLANAR)")
                print("   This means: \(totalFloat32Values / Int(channels)) samples per channel in separate planes")
            } else {
                print("   ✅ AUDIO IS INTERLEAVED")
                print("   Frames: \(totalFloat32Values / Int(channels)), Interleaved samples: \(totalFloat32Values)")
            }
        }

        // Log every 1000th callback with audio level verification
        if callbackCount % 1000 == 0 {
            let totalFloat32Values = length / MemoryLayout<Float>.size
            let actualFrames = totalFloat32Values / Int(channels)
            print("🎵 SCREEN_CAPTURE_KIT_SWIFT: Callback #\(callbackCount) - \(actualFrames) frames (\(totalFloat32Values) interleaved samples)")
        }

        // Convert to Float32 if needed
        let isFloat = (asbd.mFormatFlags & kAudioFormatFlagIsFloat) != 0
        let is32Bit = asbd.mBitsPerChannel == 32

        if isFloat && is32Bit {
            // Already Float32
            let floatPointer = dataPointer.withMemoryRebound(to: Float.self, capacity: length / MemoryLayout<Float>.size) { $0 }
            let totalSamples = length / MemoryLayout<Float>.size

            // Check if audio is non-interleaved (planar)
            let isNonInterleaved = (asbd.mFormatFlags & kAudioFormatFlagIsNonInterleaved) != 0

            if isNonInterleaved && channels == 2 {
                // Convert planar to interleaved
                let framesPerChannel = totalSamples / Int(channels)
                var interleavedBuffer = [Float](repeating: 0.0, count: totalSamples)

                // Planar layout: [L L L L ... L] [R R R R ... R]
                // Interleaved layout: [L R L R L R ... L R]
                for frame in 0..<framesPerChannel {
                    interleavedBuffer[frame * 2] = floatPointer[frame]                      // Left
                    interleavedBuffer[frame * 2 + 1] = floatPointer[framesPerChannel + frame]  // Right
                }

                if callbackCount == 1 {
                    print("   🔄 Converting planar to interleaved (\(framesPerChannel) frames × \(channels) channels)")
                    print("   First 10 interleaved samples: \(Array(interleavedBuffer.prefix(10)))")
                }

                interleavedBuffer.withUnsafeBufferPointer { buffer in
                    audioCallback(context, buffer.baseAddress, Int32(totalSamples), channels, sampleRate)
                }
            } else {
                // Already interleaved or mono, send directly
                if callbackCount == 1 {
                    let first10 = (0..<min(10, totalSamples)).map { floatPointer[$0] }
                    print("   First 10 samples (already interleaved): \(first10)")
                }

                audioCallback(context, floatPointer, Int32(totalSamples), channels, sampleRate)
            }
        } else {
            // Need conversion - convert Int16 to Float32
            let int16Pointer = dataPointer.withMemoryRebound(to: Int16.self, capacity: length / MemoryLayout<Int16>.size) { $0 }
            let totalSamples = length / MemoryLayout<Int16>.size

            var floatSamples = [Float](repeating: 0.0, count: totalSamples)
            for i in 0..<totalSamples {
                floatSamples[i] = Float(int16Pointer[i]) / 32768.0
            }

            floatSamples.withUnsafeBufferPointer { buffer in
                let frameCount = Int32(totalSamples / Int(channels))
                audioCallback(context, buffer.baseAddress, frameCount, channels, sampleRate)
            }
        }
    }

    private func sendError(_ message: String) {
        message.withCString { cString in
            errorCallback(context, cString)
        }
    }
}

// MARK: - ScreenCaptureKit Audio Stream Manager

@objc public class ScreenCaptureAudioStream: NSObject {
    private var stream: SCStream?
    private var streamOutput: ScreenCaptureAudioOutput?
    private let pid: Int32

    public init(pid: Int32) {
        self.pid = pid
        super.init()
    }

    public func start(
        audioCallback: @escaping AudioSampleCallback,
        errorCallback: @escaping ErrorCallback,
        context: UnsafeMutableRawPointer?
    ) async throws {
        // Get shareable content
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)

        // Find application with matching PID
        guard let targetApp = content.applications.first(where: { $0.processID == self.pid }) else {
            throw NSError(domain: "ScreenCaptureAudio", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "Application with PID \(pid) not found"
            ])
        }

        print("🎯 SCREEN_CAPTURE_KIT_SWIFT: Creating audio filter for app '\(targetApp.applicationName)' (PID: \(pid))")

        // Create filter to capture ONLY this application's audio
        // We need to get a window from the target application to use as anchor
        guard let appWindow = content.windows.first(where: { $0.owningApplication?.processID == self.pid }) else {
            throw NSError(domain: "ScreenCaptureAudio", code: 2, userInfo: [
                NSLocalizedDescriptionKey: "Application with PID \(pid) has no windows"
            ])
        }

        // Create filter using the application's window - this captures that app's audio
        let filter = SCContentFilter(desktopIndependentWindow: appWindow)

        // Configure stream for audio-only capture
        let config = SCStreamConfiguration()
        config.capturesAudio = true
        config.sampleRate = 48000  // 48kHz
        config.channelCount = 2     // Stereo
        config.excludesCurrentProcessAudio = true

        print("📋 SCREEN_CAPTURE_KIT_SWIFT: Requested sample rate: \(config.sampleRate)Hz, channels: \(config.channelCount)")

        // Don't capture video
        config.width = 1
        config.height = 1
        config.minimumFrameInterval = CMTime(value: 1, timescale: 1)
        config.queueDepth = 5

        // Create stream output delegate
        let output = ScreenCaptureAudioOutput(
            audioCallback: audioCallback,
            errorCallback: errorCallback,
            context: context
        )
        self.streamOutput = output

        // Create and start stream
        let stream = SCStream(filter: filter, configuration: config, delegate: nil)
        try stream.addStreamOutput(output, type: SCStreamOutputType.audio, sampleHandlerQueue: DispatchQueue.global(qos: .userInteractive))
        try await stream.startCapture()

        self.stream = stream
    }

    public func stop() async throws {
        if let stream = self.stream {
            try await stream.stopCapture()
            self.stream = nil
            self.streamOutput = nil
        }
    }

    deinit {
        // Synchronous cleanup in deinit
        if let stream = self.stream {
            Task {
                try? await stream.stopCapture()
            }
        }
    }
}

// MARK: - C API for Rust FFI

@_cdecl("sc_audio_get_available_applications")
public func sc_audio_get_available_applications(
    outApps: UnsafeMutablePointer<UnsafeMutablePointer<UnsafeMutablePointer<SCAppInfo>?>?>?,
    outCount: UnsafeMutablePointer<Int32>?
) -> Int32 {
    print("🔍 SCREEN_CAPTURE_KIT_SWIFT: Getting available applications...")

    let semaphore = DispatchSemaphore(value: 0)
    var apps: [SCAppInfo] = []
    var errorCode: Int32 = 0
    var errorMessage: String?

    Task {
        do {
            print("📡 SCREEN_CAPTURE_KIT_SWIFT: Requesting shareable content...")
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            print("✅ SCREEN_CAPTURE_KIT_SWIFT: Got \(content.applications.count) applications")

            apps = content.applications.map { app in
                return SCAppInfo(
                    pid: app.processID,
                    bundleIdentifier: app.bundleIdentifier,
                    applicationName: app.applicationName
                )
            }
            errorCode = 0
        } catch {
            print("❌ SCREEN_CAPTURE_KIT_SWIFT error: \(error.localizedDescription)")
            errorMessage = error.localizedDescription
            errorCode = -1
        }
        semaphore.signal()
    }

    let waitResult = semaphore.wait(timeout: .now() + 10.0)

    if waitResult == .timedOut {
        print("⏱️ SCREEN_CAPTURE_KIT_SWIFT: Timed out waiting for content")
        return -2
    }

    if errorCode != 0 {
        if let msg = errorMessage {
            print("❌ SCREEN_CAPTURE_KIT_SWIFT failed: \(msg)")
        }
        return errorCode
    }

    if apps.isEmpty {
        print("⚠️ SCREEN_CAPTURE_KIT_SWIFT: No applications found")
        outApps?.pointee = nil
        outCount?.pointee = 0
        return 0
    }

    // Allocate array of pointers
    let appsArray = UnsafeMutablePointer<UnsafeMutablePointer<SCAppInfo>?>.allocate(capacity: apps.count)
    for (index, app) in apps.enumerated() {
        appsArray[index] = Unmanaged.passRetained(app).toOpaque().bindMemory(to: SCAppInfo.self, capacity: 1)
    }

    outApps?.pointee = appsArray
    outCount?.pointee = Int32(apps.count)

    print("✅ SCREEN_CAPTURE_KIT_SWIFT: Returning \(apps.count) applications")
    return 0
}

@_cdecl("sc_audio_free_applications")
public func sc_audio_free_applications(apps: UnsafeMutablePointer<UnsafeMutablePointer<SCAppInfo>?>?, count: Int32) {
    guard let apps = apps else { return }

    for i in 0..<Int(count) {
        if let appPtr = apps[i] {
            Unmanaged<SCAppInfo>.fromOpaque(appPtr).release()
        }
    }

    apps.deallocate()
}

// Accessor functions for SCAppInfo fields
@_cdecl("sc_audio_app_get_pid")
public func sc_audio_app_get_pid(appPtr: UnsafeMutableRawPointer?) -> Int32 {
    guard let appPtr = appPtr else { return -1 }
    let app = Unmanaged<SCAppInfo>.fromOpaque(appPtr).takeUnretainedValue()
    return app.pid
}

@_cdecl("sc_audio_app_get_bundle_id")
public func sc_audio_app_get_bundle_id(appPtr: UnsafeMutableRawPointer?) -> UnsafePointer<CChar>? {
    guard let appPtr = appPtr else { return nil }
    let app = Unmanaged<SCAppInfo>.fromOpaque(appPtr).takeUnretainedValue()
    return (app.bundleIdentifier as NSString).utf8String
}

@_cdecl("sc_audio_app_get_name")
public func sc_audio_app_get_name(appPtr: UnsafeMutableRawPointer?) -> UnsafePointer<CChar>? {
    guard let appPtr = appPtr else { return nil }
    let app = Unmanaged<SCAppInfo>.fromOpaque(appPtr).takeUnretainedValue()
    return (app.applicationName as NSString).utf8String
}

@_cdecl("sc_audio_stream_create")
public func sc_audio_stream_create(pid: Int32) -> UnsafeMutableRawPointer? {
    let stream = ScreenCaptureAudioStream(pid: pid)
    return Unmanaged.passRetained(stream).toOpaque()
}

@_cdecl("sc_audio_stream_start")
public func sc_audio_stream_start(
    streamPtr: UnsafeMutableRawPointer?,
    audioCallback: @escaping AudioSampleCallback,
    errorCallback: @escaping ErrorCallback,
    context: UnsafeMutableRawPointer?
) -> Int32 {
    guard let streamPtr = streamPtr else { return -1 }

    let stream = Unmanaged<ScreenCaptureAudioStream>.fromOpaque(streamPtr).takeUnretainedValue()

    let semaphore = DispatchSemaphore(value: 0)
    var result: Int32 = 0

    Task {
        do {
            try await stream.start(
                audioCallback: audioCallback,
                errorCallback: errorCallback,
                context: context
            )
            result = 0
        } catch {
            let errorMsg = error.localizedDescription
            errorMsg.withCString { cString in
                errorCallback(context, cString)
            }
            result = -1
        }
        semaphore.signal()
    }

    semaphore.wait()
    return result
}

@_cdecl("sc_audio_stream_stop")
public func sc_audio_stream_stop(streamPtr: UnsafeMutableRawPointer?) -> Int32 {
    guard let streamPtr = streamPtr else { return -1 }

    let stream = Unmanaged<ScreenCaptureAudioStream>.fromOpaque(streamPtr).takeUnretainedValue()

    let semaphore = DispatchSemaphore(value: 0)
    var result: Int32 = 0

    Task {
        do {
            try await stream.stop()
            result = 0
        } catch {
            result = -1
        }
        semaphore.signal()
    }

    semaphore.wait()
    return result
}

@_cdecl("sc_audio_stream_destroy")
public func sc_audio_stream_destroy(streamPtr: UnsafeMutableRawPointer?) {
    guard let streamPtr = streamPtr else { return }
    Unmanaged<ScreenCaptureAudioStream>.fromOpaque(streamPtr).release()
}

@_cdecl("sc_audio_check_permission")
public func sc_audio_check_permission() -> Int32 {
    if #available(macOS 13.0, *) {
        // ScreenCaptureKit is available, but we can't directly check permission
        // Permission is requested when starting capture
        return 1
    } else {
        return 0
    }
}
