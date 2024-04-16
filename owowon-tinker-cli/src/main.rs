use owowon::device::Device;
use std::time::Instant;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device = Device::from_first_vid_pid_match().await?;
    let mut io = device.raw_io()?;

    let mut buf = [0u8; 10240];

    for line in std::io::stdin().lines() {
        let line = line.unwrap();
        if line.is_empty() {
            break;
        }

        let time = Instant::now();

        io.raw_send_nowait(line.as_bytes()).await?;

        if line.ends_with('?') {
            let msg = io.recv(&mut buf).await?;

            println!("{}", pretty_hex::pretty_hex(&msg));
        }

        println!("{} ms", time.elapsed().as_millis());
    }

    Ok(())
}
