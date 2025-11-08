use anyhow::{Context, Ok, Result};
use futures_util::{StreamExt, future::join_all};
use scraper::{Html, Selector};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufWriter};
use urlencoding::decode;

#[tokio::main]
async fn main() -> Result<()> {
    let client: reqwest::Client = reqwest::Client::new();
    let a: Vec<String> = init_page_scrape("https://downloads.khinsider.com/game-soundtracks/album/cyberpunk-2077-phantom-liberty-original-score-deluxe-edition-2023", &client).await?;
    let b: Vec<String> = down_page_scrape(a, &client).await?;
    download_tracks(b, &client).await?;
    Ok(())
}

async fn fetch_html(link: &str, client: &reqwest::Client) -> Result<Html> {
    println!("Fetching {link}");
    let res: reqwest::Response = client
        .get(link)
        .send()
        .await
        .context("[ERROR] GET request failed.")?;
    let status = res.status();
    if status == 200 {
        let html = res
            .text()
            .await
            .context("[ERROR] Failed to get HTML data.")?;
        let parsed = scraper::Html::parse_document(&html);
        Ok(parsed)
    } else {
        eprintln!("HTTP {status} failed to fetch.");
        std::process::exit(1);
    }
}
async fn init_page_scrape(link: &str, client: &reqwest::Client) -> Result<Vec<String>> {
    println!("(Stage 1) Getting the album page.");
    let parsed = fetch_html(link, client).await?;
    let selector: Selector = Selector::parse(r#"td.playlistDownloadSong a[href*=".mp3"]"#).unwrap();
    let a = parsed
        .select(&selector)
        .filter_map(|x| {
            x.value()
                .attr("href")
                .map(|x| format!("https://downloads.khinsider.com/{x}"))
        })
        .collect();
    Ok(a)
}
async fn down_page_scrape(downlist: Vec<String>, client: &reqwest::Client) -> Result<Vec<String>> {
    println!("(Stage 2) Getting the audio file links.");
    let b: Vec<Html> = join_all(downlist.iter().map(|link| fetch_html(&link, client)))
        .await
        .iter()
        .map(|result| result.as_ref().unwrap())
        .cloned()
        .collect();
    let selector: Selector = Selector::parse(r#"a:has(span.songDownloadLink)"#).unwrap();
    let a = b
        .iter()
        .map(|x: &Html| {
            x.select(&selector)
                .filter_map(|x: scraper::ElementRef<'_>| x.value().attr("href"))
                .filter(|x: &&str| x.ends_with(".flac"))
                .map(|x| x.to_string())
                .collect()
        })
        .collect();
    Ok(a)
}
async fn download_tracks(downlist: Vec<String>, client: &reqwest::Client) -> Result<()> {
    println!("Stage 3 : Downloading files.");
    'outer: for track in downlist {
        let name = decode(track.split("/").last().unwrap())
            .unwrap()
            .into_owned();

        println!("Download {name} y/n/q?");
        let mut user_string = String::new();
        loop {
            user_string.clear();
            let mut stdin = io::BufReader::new(io::stdin());
            stdin.read_line(&mut user_string).await?;
            match user_string.trim().to_lowercase().as_str() {
                "y" => break,
                "n" => continue 'outer,
                "q" => return Ok(()),
                _ => {
                    println!("oi, you didn't enter a proper option, try again.")
                }
            }
        }
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
