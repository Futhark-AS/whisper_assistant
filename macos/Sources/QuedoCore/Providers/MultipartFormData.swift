import Foundation

struct MultipartFormData {
    let boundary: String
    private var body = Data()

    init() {
        boundary = "Boundary-\(UUID().uuidString)"
    }

    mutating func addField(name: String, value: String) {
        append("--\(boundary)\r\n")
        append("Content-Disposition: form-data; name=\"\(name)\"\r\n\r\n")
        append("\(value)\r\n")
    }

    mutating func addFile(name: String, filename: String, mimeType: String, data: Data) {
        append("--\(boundary)\r\n")
        append("Content-Disposition: form-data; name=\"\(name)\"; filename=\"\(filename)\"\r\n")
        append("Content-Type: \(mimeType)\r\n\r\n")
        body.append(data)
        append("\r\n")
    }

    mutating func finalize() {
        append("--\(boundary)--\r\n")
    }

    func data() -> Data {
        body
    }

    private mutating func append(_ string: String) {
        guard let data = string.data(using: .utf8) else {
            return
        }
        body.append(data)
    }
}
