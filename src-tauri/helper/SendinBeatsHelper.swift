import Foundation
import Security

let driverBundleName = "SendinBeatsAudioDriver.bundle"
let driverDestination = "/Library/Audio/Plug-Ins/HAL/\(driverBundleName)"
let socketPath = "/tmp/sendin_beats_helper.sock"

// MARK: - Driver Installation

func installDriver(sourcePath: String) -> Bool {
    let fileManager = FileManager.default

    // Check if already installed
    if fileManager.fileExists(atPath: driverDestination) {
        print("[Helper] Driver already installed at \(driverDestination)")
        return true
    }

    // Verify source exists
    guard fileManager.fileExists(atPath: sourcePath) else {
        print("[Helper] ERROR: Source driver not found at \(sourcePath)")
        return false
    }

    print("[Helper] Installing driver from \(sourcePath) to \(driverDestination)")

    // Create HAL directory if it doesn't exist
    let halDirectory = "/Library/Audio/Plug-Ins/HAL"
    if !fileManager.fileExists(atPath: halDirectory) {
        do {
            try fileManager.createDirectory(atPath: halDirectory, withIntermediateDirectories: true, attributes: nil)
            print("[Helper] Created HAL directory")
        } catch {
            print("[Helper] ERROR: Failed to create HAL directory: \(error)")
            return false
        }
    }

    // Copy driver bundle
    do {
        try fileManager.copyItem(atPath: sourcePath, toPath: driverDestination)
        print("[Helper] Driver installed successfully")

        // Restart CoreAudio to load the new driver
        restartCoreAudio()

        return true
    } catch {
        print("[Helper] ERROR: Failed to install driver: \(error)")
        return false
    }
}

func uninstallDriver() -> Bool {
    let fileManager = FileManager.default

    guard fileManager.fileExists(atPath: driverDestination) else {
        print("[Helper] Driver not installed")
        return true
    }

    print("[Helper] Uninstalling driver from \(driverDestination)")

    do {
        try fileManager.removeItem(atPath: driverDestination)
        print("[Helper] Driver uninstalled successfully")

        // Restart CoreAudio
        restartCoreAudio()

        return true
    } catch {
        print("[Helper] ERROR: Failed to uninstall driver: \(error)")
        return false
    }
}

func restartCoreAudio() {
    print("[Helper] Restarting CoreAudio daemon...")

    let task = Process()
    task.launchPath = "/usr/bin/killall"
    task.arguments = ["coreaudiod"]

    do {
        try task.run()
        task.waitUntilExit()

        if task.terminationStatus == 0 {
            print("[Helper] CoreAudio restarted successfully")
            // Give CoreAudio time to restart
            sleep(2)
        } else {
            print("[Helper] WARNING: CoreAudio restart returned status \(task.terminationStatus)")
        }
    } catch {
        print("[Helper] ERROR: Failed to restart CoreAudio: \(error)")
    }
}

// MARK: - IPC Server

// Message format: [command: 1 byte][pid: 4 bytes][channel: 4 bytes]
// Commands: 0x01 = map PID to channel, 0x02 = unmap PID

@_cdecl("MapPIDToChannel")
func MapPIDToChannel(_ pid: Int32, _ channel: Int32) {
    // This function signature matches the C driver's expectation
    // In practice, we'd load the driver's dylib and call into it
    // For now, this is a placeholder that will be linked with the driver
    print("[Helper] Map PID \(pid) -> channel \(channel)")
}

@_cdecl("UnmapPID")
func UnmapPID(_ pid: Int32) {
    print("[Helper] Unmap PID \(pid)")
}

func startIPCServer() {
    print("[Helper] Starting IPC server on \(socketPath)")

    let fileManager = FileManager.default

    // Remove existing socket if present
    if fileManager.fileExists(atPath: socketPath) {
        do {
            try fileManager.removeItem(atPath: socketPath)
        } catch {
            print("[Helper] WARNING: Failed to remove existing socket: \(error)")
        }
    }

    // Create Unix domain socket
    let sock = socket(AF_UNIX, SOCK_STREAM, 0)
    guard sock >= 0 else {
        print("[Helper] ERROR: Failed to create socket")
        return
    }

    var addr = sockaddr_un()
    addr.sun_family = sa_family_t(AF_UNIX)
    _ = socketPath.withCString { cstr in
        withUnsafeMutableBytes(of: &addr.sun_path) { pathBytes in
            let pathPtr = pathBytes.baseAddress!.assumingMemoryBound(to: Int8.self)
            strncpy(pathPtr, cstr, pathBytes.count)
        }
    }

    let addrLen = socklen_t(MemoryLayout<UInt8>.size + MemoryLayout<sa_family_t>.size + strlen(socketPath) + 1)

    let bindResult = withUnsafePointer(to: &addr) { addrPtr in
        addrPtr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPtr in
            bind(sock, sockaddrPtr, addrLen)
        }
    }

    guard bindResult >= 0 else {
        print("[Helper] ERROR: Failed to bind socket")
        close(sock)
        return
    }

    guard listen(sock, 5) >= 0 else {
        print("[Helper] ERROR: Failed to listen on socket")
        close(sock)
        return
    }

    print("[Helper] IPC server listening for connections")

    // Accept connections in a loop
    while true {
        let client = accept(sock, nil, nil)
        if client < 0 {
            print("[Helper] WARNING: Failed to accept connection")
            continue
        }

        // Handle client in background
        DispatchQueue.global(qos: .userInitiated).async {
            handleClient(clientSocket: client)
        }
    }
}

func handleClient(clientSocket: Int32) {
    defer {
        close(clientSocket)
    }

    var buffer = [UInt8](repeating: 0, count: 9)
    let bytesRead = read(clientSocket, &buffer, 9)

    guard bytesRead == 9 else {
        print("[Helper] WARNING: Invalid message size: \(bytesRead)")
        return
    }

    let command = buffer[0]
    let pid = Int32(buffer[1]) << 24 | Int32(buffer[2]) << 16 | Int32(buffer[3]) << 8 | Int32(buffer[4])
    let channel = Int32(buffer[5]) << 24 | Int32(buffer[6]) << 16 | Int32(buffer[7]) << 8 | Int32(buffer[8])

    switch command {
    case 0x01: // Map PID to channel
        print("[Helper] Received map request: PID \(pid) -> channel \(channel)")
        MapPIDToChannel(pid, channel)

    case 0x02: // Unmap PID
        print("[Helper] Received unmap request: PID \(pid)")
        UnmapPID(pid)

    default:
        print("[Helper] WARNING: Unknown command: 0x\(String(format: "%02X", command))")
    }

    // Send acknowledgment
    var response: UInt8 = 0x00 // Success
    write(clientSocket, &response, 1)
}

// MARK: - Main Entry Point

func printUsage() {
    print("""
    Sendin Beats Audio Helper

    Usage:
        sendin-beats-helper install <driver-path>   Install the audio driver
        sendin-beats-helper uninstall               Uninstall the audio driver
        sendin-beats-helper daemon                  Run IPC daemon for PID routing

    The helper must be run with root privileges to install/uninstall drivers.
    """)
}

func main() {
    let args = CommandLine.arguments

    guard args.count >= 2 else {
        printUsage()
        exit(1)
    }

    let command = args[1]

    switch command {
    case "install":
        guard args.count >= 3 else {
            print("[Helper] ERROR: install command requires driver path")
            printUsage()
            exit(1)
        }

        let driverPath = args[2]
        let success = installDriver(sourcePath: driverPath)
        exit(success ? 0 : 1)

    case "uninstall":
        let success = uninstallDriver()
        exit(success ? 0 : 1)

    case "daemon":
        print("[Helper] Starting IPC daemon...")
        startIPCServer()

    default:
        print("[Helper] ERROR: Unknown command '\(command)'")
        printUsage()
        exit(1)
    }
}

// Run main
main()
