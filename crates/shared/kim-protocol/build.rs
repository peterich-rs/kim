fn main() {
    std::env::set_var("PROTOC", protobuf_src::protoc());
    prost_build::Config::new()
        .compile_protos(&["proto/pkt.proto", "proto/agent.proto"], &["proto"])
        .expect("compile pkt.proto and agent.proto");
}
