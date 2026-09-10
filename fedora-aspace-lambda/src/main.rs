use fedora_tools::fedora_queue::{FedoraURIs, get_messages};

#[tokio::main]
async fn main() {
    let url = "localhost:61613";
    let result = get_messages(url, "/queue/fedora").await.unwrap();
    println!("{:?}", result);
}
