use async_nats;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = async_nats::connect("nats://localhost:4222").await?;
    println!("Connected to NATS. Listening for domain.* events...");
    
    let mut subscriber = client.subscribe("domain.*").await?;
    
    while let Some(msg) = subscriber.next().await {
        println!("\n=== Received on '{}' ===", msg.subject);
        println!("{}", String::from_utf8_lossy(&msg.payload));
    }
    
    Ok(())
}
