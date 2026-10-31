import Foundation

/// Sends one HTTP request; the seam the client tests replace.
public protocol HTTPTransport: Sendable {
    /// - Throws: `URLError` when nothing answers in time, `CancellationError` when cancelled.
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

/// The real transport: an ephemeral session that never waits for connectivity, so a stopped
/// allotd shows as refused at once.
public struct URLSessionTransport: HTTPTransport {
    private let session: URLSession

    public init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.waitsForConnectivity = false
        configuration.urlCache = nil
        // allotd is on loopback; a system proxy must never see the token.
        configuration.connectionProxyDictionary = [:]
        session = URLSession(configuration: configuration)
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw URLError(.badServerResponse)
        }
        return (data, http)
    }
}
