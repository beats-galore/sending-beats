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

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio else { return }

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

        // Convert to Float32 if needed
        let isFloat = (asbd.mFormatFlags & kAudioFormatFlagIsFloat) != 0
        let is32Bit = asbd.mBitsPerChannel == 32

        if isFloat && is32Bit {
            // Already Float32, send directly
            let floatPointer = dataPointer.withMemoryRebound(to: Float.self, capacity: length / MemoryLayout<Float>.size) { $0 }
            let sampleCount = Int32(length / MemoryLayout<Float>.size)

            audioCallback(context, floatPointer, sampleCount, channels, sampleRate)
        } else {
            // Need conversion - convert Int16 to Float32
            let int16Pointer = dataPointer.withMemoryRebound(to: Int16.self, capacity: length / MemoryLayout<Int16>.size) { $0 }
            let sampleCount = length / MemoryLayout<Int16>.size

            var floatSamples = [Float](repeating: 0.0, count: sampleCount)
            for i in 0..<sampleCount {
                floatSamples[i] = Float(int16Pointer[i]) / 32768.0
            }

            floatSamples.withUnsafeBufferPointer { buffer in
                audioCallback(context, buffer.baseAddress, Int32(sampleCount), channels, sampleRate)
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
        guard let app = content.applications.first(where: { $0.processID == self.pid }) else {
            throw NSError(domain: "ScreenCaptureAudio", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "Application with PID \(pid) not found"
            ])
        }

        // Create filter for this specific application (audio only)
        // Use first window as placeholder - audio capture is app-wide anyway
        let filter = SCContentFilter(desktopIndependentWindow: content.windows.first!)

        // Configure stream for audio-only capture
        let config = SCStreamConfiguration()
        config.capturesAudio = true
        config.sampleRate = 48000  // 48kHz
        config.channelCount = 2     // Stereo
        config.excludesCurrentProcessAudio = true

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
    let semaphore = DispatchSemaphore(value: 0)
    var apps: [SCAppInfo] = []
    var errorCode: Int32 = 0

    Task {
        do {
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)

            apps = content.applications.map { app in
                return SCAppInfo(
                    pid: app.processID,
                    bundleIdentifier: app.bundleIdentifier,
                    applicationName: app.applicationName
                )
            }
            errorCode = 0
        } catch {
            errorCode = -1
        }
        semaphore.signal()
    }

    semaphore.wait()

    if errorCode != 0 {
        return errorCode
    }

    // Allocate array of pointers
    let appsArray = UnsafeMutablePointer<UnsafeMutablePointer<SCAppInfo>?>.allocate(capacity: apps.count)
    for (index, app) in apps.enumerated() {
        appsArray[index] = Unmanaged.passRetained(app).toOpaque().bindMemory(to: SCAppInfo.self, capacity: 1)
    }

    outApps?.pointee = appsArray
    outCount?.pointee = Int32(apps.count)

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
