use anyhow::{Ok, Result};
use futures_util::StreamExt;
use scraper::Selector;
use tokio::io::{AsyncWriteExt, BufWriter};
use urlencoding::decode;

#[tokio::main]
async fn main() -> Result<()> {
    let client: reqwest::Client = reqwest::Client::new();
    let a: Vec<String> = init_page_scrape("https://downloads.khinsider.com/game-soundtracks/album/risk-of-rain-2-survivors-of-the-void-2022", &client).await?;
    let b: Vec<String> = down_page_scrape(a, &client).await?;
    download_tracks(b, &client).await?;
    Ok(())
}

async fn init_page_scrape(link: &'static str, client: &reqwest::Client) -> Result<Vec<String>> {
    let mut a: Vec<String> = Vec::new();
    let res: reqwest::Response = client.get(link).send().await.unwrap();
    println!("Status: {}", res.status());
    let html: String = res.text().await?;
    println!("Parsing...");
    let parsed: scraper::Html = scraper::Html::parse_document(&html);
    let selector: Selector = Selector::parse("td.playlistDownloadSong a[href]").unwrap();
    for element in parsed.select(&selector) {
        a.push(
            "https://downloads.khinsider.com/".to_string()
                + &element.value().attr("href").unwrap().to_string(),
        );
    }
    Ok(a)
}
async fn down_page_scrape(downlist: Vec<String>, client: &reqwest::Client) -> Result<Vec<String>> {
    let mut a: Vec<String> = Vec::new();
    for link in downlist {
        let res: reqwest::Response = client.get(link).send().await?;
        let html: String = res.text().await?;
        let parsed: scraper::Html = scraper::Html::parse_document(&html);
        let selector: Selector = Selector::parse("a[href]").unwrap();
        for element in parsed.select(&selector) {
            if let Some(href) = element.value().attr("href") {
                if href.ends_with(".flac") {
                    a.push(href.to_string());
                }
            }
        }
    }
    Ok(a)
}
async fn download_tracks(downlist: Vec<String>, client: &reqwest::Client) -> Result<()> {
    for track in downlist {
        let name = decode(track.split("/").last().unwrap())
            .unwrap()
            .into_owned();
        let inner: tokio::fs::File = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&name)
            .await
            .unwrap();
        let mut writer: BufWriter<tokio::fs::File> = BufWriter::new(inner);
        let file: reqwest::Response = client.get(&track).send().await?;
        let mut file_stream = file.bytes_stream();
        while let Some(a) = file_stream.next().await {
            writer.write_all(&a.unwrap()).await?;
        }
        println!("file downloaded. {0}", name);
    }
    Ok(())
}
