
# hidden-pixel final report

Hidden pixel can now transmit an arbitrary, encrypted, hidden sequence of bytes embedded in the intra-prediction angles inside an AV1 stream. I started by implementing the diff from the [original paper](https://web.archive.org/web/20220531053704/https://files.catbox.moe/e3f61j.pdf) in new forks of the [dav1d](https://code.videolan.org/videolan/dav1d) decoder ([my fork](https://github.com/TroyNeubauer/dav1d)), the [rav1e](https://github.com/xiph/rav1e) encoder ([my fork](https://github.com/TroyNeubauer/rav1e)). Aside from some minor differences, the diff was applied easily on the lastest master branches, allowing me to take advantage of the performance improvements and bug fixes added in the last three years since the paper came out. 

From there I looked into open source RTSP server implementations than supported AV1 so I could splice in my key exchange and stenography logic. 
Unfortunately I couldn't find any viable options that already had support for AV1, and would be easy to hack on. 
Due to this, I decided to write my own minimal "RTSP like" [server](https://github.com/TroyNeubauer/rav1e/compare/master..steg#diff-6afcea77448a5d32fd8b38bdbce707d55813b468f3f518204d084f235d98c490R1-R797). This server has three accepts three basic commands via a binary serialization scheme:

| Opcode (8 bits)        | Data 1 (... bits)                | Data 2 (... bits) |
| --------               | -------                          | -------           |
| 0 - Set Parameter      | Value Length, N (16 bits LSB)    | Value (N bytes)   |
| 1 - Begin Video Stream | N/A                              | N/A               |
| 2 - Stop Video Stream  | N/A                              | N/A               |

Begin Video Stream and Stop Video Stream are single 8 bit opcodes with no associated data. Set Parameter is followed by a 16 bit length field encoded in little endian. This is followed by N bytes (0..65535, depending on previous length field).

# The Secret Handshake

The secret handshake begins when a client initiates a TCP connection to the server and calls Set Parameter with a 256 bit x25519 public key. Next the client calls Begin Video Stream. In the background the server recognizes this secret handshake and generates its own public and privates x25519 keys, using the client's public key to derive a shared secret. The server begin video streaming with stenography active.
The first 256 bits injected in the infra-prediction angles are the server's public key, which the client uses with its private key to compute the same shared secret. After the server's public key is transmitted, the payload data is encrypted using the shared secret and the ChaCha20 stream cipher. The cipher text is then injected into the angles as the video is transmitted to the client. The client extracts the cipher text from the doctored angles, and decrypts the payload using the same shared key and algorithm, printing the secret message as it is streamed. 

# Drawbacks

The dav1d decoder [doesn't support reading from stdin](https://code.videolan.org/videolan/dav1d/-/issues/286). This is problematic, since dav1d is a command line utility designed to only operate on files, it has no support for streaming. The easiest way to get the AV1 stream to dav1d is to copy the tcp video stream from the server to dav1d's stdin. I was able to work around this by applying this [unmerged patch](https://code.videolan.org/videolan/dav1d/-/merge_requests/1188?commit_id=54166c5d6ef946464c7bca0b8d52f9190a2f8406), however this copies all of stdin to a file before starting the decoding process, making real-time streaming impossible. dav1d uses seeking extensively to initialize its data structures on startup, which only only implemented on files.  

