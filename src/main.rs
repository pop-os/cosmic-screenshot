use ashpd::desktop::screenshot::Screenshot;
use std::fs;

//TODO: better error handling
#[tokio::main]
async fn main() {
    let picture_dir = dirs::picture_dir().expect("failed to locate picture directory");

    let date = chrono::Local::now();
    let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
    let path = picture_dir.join(filename);

    let response = Screenshot::request()
        .interactive(true)
        .modal(true)
        .send()
        .await
        .expect("failed to send screenshot request")
        .response()
        .expect("failed to receive screenshot response");

    let uri = response.uri();
    let tmp_path = match uri.scheme() {
        "file" => uri.path(),
        scheme => panic!("unsupported scheme '{}'", scheme),
    };

    fs::copy(tmp_path, &path).expect("failed to copy screenshot file");

    println!("{}", path.display());
}
