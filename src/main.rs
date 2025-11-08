use anyhow::{Context, Ok, Result};
use futures_util::{StreamExt, future::join_all};
use scraper::{Html, Selector};
use std::env;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufWriter};
use urlencoding::decode;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let reqwest_client: reqwest::Client = reqwest::Client::new();
    if args.len() < 2 {
        println!("[ERROR] No link provided.");
        println!("[USAGE] khid_rust.exe <link>.");
        Ok(())
    } else {
        let init_p_links: Vec<String> = init_page_scrape(&args[2], &reqwest_client).await?;
        let down_p_links: Vec<String> = down_page_scrape(init_p_links, &reqwest_client).await?;
        download_tracks(down_p_links, &reqwest_client).await?;
        Ok(())
    }
}

async fn fetch_html(link: &str, client: &reqwest::Client) -> Result<Html> {
    let response: reqwest::Response = client
        .get(link)
        .send()
        .await
        .context("[ERROR] GET request failed.")?;
    let status_code = response.status();
    if status_code == 200 {
        let html = response
            .text()
            .await
            .context("[ERROR] Failed to get HTML data.")?;
        let parsed_html = scraper::Html::parse_document(&html);
        Ok(parsed_html)
    } else {
        eprintln!("[ERROR] HTTP {status_code} failed to fetch.");
        std::process::exit(1);
    }
}
async fn init_page_scrape(link: &str, client: &reqwest::Client) -> Result<Vec<String>> {
    println!("[Stage 1] Getting the album page.");
    let parsed_html = fetch_html(link, client).await?;
    let selector: Selector = Selector::parse(r#"td.playlistDownloadSong a[href*=".mp3"]"#).unwrap();
    let links = parsed_html
        .select(&selector)
        .filter_map(|x| {
            x.value()
                .attr("href")
                .map(|x| format!("https://downloads.khinsider.com/{x}"))
        })
        .collect();
    Ok(links)
}
async fn down_page_scrape(downlist: Vec<String>, client: &reqwest::Client) -> Result<Vec<String>> {
    println!("[Stage 2] Getting the audio file links.");
    let html_list: Vec<Html> = join_all(downlist.iter().map(|link| fetch_html(&link, client)))
        .await
        .iter()
        .map(|result| result.as_ref().unwrap())
        .cloned()
        .collect();
    let selector: Selector = Selector::parse(r#"a:has(span.songDownloadLink)"#).unwrap();
    let links = html_list
        .iter()
        .map(|x: &Html| {
            x.select(&selector)
                .filter_map(|x: scraper::ElementRef<'_>| x.value().attr("href"))
                .filter(|x: &&str| x.ends_with(".flac"))
                .map(|x| x.to_string())
                .collect()
        })
        .collect();
    Ok(links)
}
async fn download_tracks(links: Vec<String>, client: &reqwest::Client) -> Result<()> {
    println!("[Stage 3] Downloading files.");
    let mut stdin_handle = io::BufReader::new(io::stdin());
    'outer: for link in links {
        let track_name = decode(link.split("/").last().unwrap())
            .unwrap()
            .into_owned();
        println!("[DOWNLOAD] \"{track_name}\" [y/n/q]?");
        let mut user_input_buffer = String::new();

        loop {
            user_input_buffer.clear();
            stdin_handle.read_line(&mut user_input_buffer).await?;
            match user_input_buffer.trim().to_lowercase().as_str() {
                "y" => break,
                "n" => continue 'outer,
                "q" => return Ok(()),
                _ => {
                    println!("[ERROR] You didn't enter a proper option! Try again.")
                }
            }
        }

        let tokio_openoptions: tokio::fs::File = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&track_name)
            .await
            .unwrap();
        let mut buf_writer: BufWriter<tokio::fs::File> = BufWriter::new(tokio_openoptions);
        let target_file: reqwest::Response = client.get(&link).send().await?;
        let mut file_stream = target_file.bytes_stream();

        while let Some(a) = file_stream.next().await {
            buf_writer.write_all(&a.unwrap()).await?;
        }
        println!("[INFO] File {0} downloaded successfully.", track_name);
    }
    Ok(())
}
