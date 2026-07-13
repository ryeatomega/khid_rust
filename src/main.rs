use anyhow::{Context, Ok, Result};
use colored::Colorize;
use futures_util::{StreamExt, future::join_all};
use scraper::{Html, Selector};
use std::{
    env,
    io::{self as stdio, Write},
    vec,
};
use tokio::io::{self as tokio_io};
use urlencoding::decode;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("{} No link provided!", "[ERROR]".red().bold());
        println!("{} khid_rust <album url>", "[USAGE]".yellow().bold());
        return Ok(());
    }

    let reqwest_client: reqwest::Client = reqwest::Client::new();

    let init_p_links = init_page_scrape(&args[1], &reqwest_client).await?;

    if init_p_links.len() < 1 {
        eprintln!("{} Failed to get links", "[ERROR]".red().bold());
        return Ok(());
    }

    let down_p_links = down_page_scrape(init_p_links, &reqwest_client)
        .await
        .context(format!(
            "{} Failed to get download links",
            "[ERROR]".red().bold()
        ));

    let _ = download_tracks(down_p_links.unwrap(), &reqwest_client)
        .await
        .context(format!(
            "{} Failed to download the tracks.",
            "[ERROR]".red().bold()
        ));

    Ok(())
}

async fn fetch_html(link: &str, client: &reqwest::Client) -> Result<Html> {
    let response = client
        .get(link)
        .send()
        .await
        .context(format!("{} GET request failed.", "[ERROR]".red().bold()))?
        .error_for_status()
        .context(format!("{} HTTP request failed.", "[ERROR]".red().bold()))?;

    let html = response.text().await.context(format!(
        "{} Failed to get HTML data.",
        "[ERROR]".red().bold()
    ))?;

    Ok(Html::parse_document(&html))
}

async fn init_page_scrape(link: &str, client: &reqwest::Client) -> Result<Vec<String>> {
    println!("{} Getting the album page.", "[Stage 1]".green().bold());

    let parsed_html = fetch_html(link, client).await?;

    let selector = Selector::parse(r#"td.playlistDownloadSong a[href*=".mp3"]"#)
        .expect("Invalid CSS selector");

    Ok(parsed_html
        .select(&selector)
        .filter_map(|element| {
            element
                .value()
                .attr("href")
                .map(|href| format!("https://downloads.khinsider.com/{href}"))
        })
        .collect())
}

async fn down_page_scrape(downlist: Vec<String>, client: &reqwest::Client) -> Result<Vec<String>> {
    println!(
        "{} Getting the audio file links.",
        "[Stage 2]".green().bold()
    );

    let html_list: Vec<Html> = join_all(downlist.iter().map(|link| fetch_html(&link, client)))
        .await
        .iter()
        .map(|result| result.as_ref().unwrap())
        .cloned()
        .collect();

    let selector: Selector =
        Selector::parse(r#"a:has(span.songDownloadLink)"#).expect("Invalid CSS selector");

    Ok(html_list
        .iter()
        .map(|page: &Html| {
            page.select(&selector)
                .filter_map(|element: scraper::ElementRef<'_>| element.value().attr("href"))
                .filter(|href: &&str| href.ends_with(".mp3"))
                .map(|href| href.to_string())
                .collect()
        })
        .collect())
}

async fn download_tracks(links: Vec<String>, client: &reqwest::Client) -> Result<()> {
    println!("{} Preparing for download.", "[Stage 3]".green().bold());

    let mut stdin_handle = stdio::BufReader::new(stdio::stdin());
    let mut dwn_list = vec![];
    let mut user_input_buffer = String::new();

    'outer: for link in links {
        let track_name = decode(link.split("/").last().expect("Failed to split the link."))
            .expect("Failed to decode url.")
            .into_owned();

        stdio::stdout().flush()?;

        print!(
            r#"{} "{track_name}" [y/n]? "#,
            "[CHOICE: DOWNLOAD?]".cyan().bold()
        );

        stdio::stdout().flush()?;

        loop {
            user_input_buffer.clear();
            stdio::BufRead::read_line(&mut stdin_handle, &mut user_input_buffer)?;
            match user_input_buffer.trim().to_lowercase().as_str() {
                "y" => break,
                "n" => continue 'outer,
                _ => {
                    println!("{} Invalid option! Try again.", "[ERROR]".red().bold())
                }
            }
        }
        dwn_list.push([track_name, link]);
    }

    println!("{} Preparing to download the list", "[INFO]".cyan().bold());

    let dwn_threads = dwn_list.iter().map(|[name, link]| async move {
        let tokio_openoptions: tokio::fs::File = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&name)
            .await
            .expect("OpenOptions failed.");

        let mut buf_writer: tokio_io::BufWriter<tokio::fs::File> =
            tokio_io::BufWriter::new(tokio_openoptions);

        let target_file: reqwest::Response = client.get(link).send().await.unwrap();

        let mut file_stream = target_file.bytes_stream();

        while let Some(chunk) = file_stream.next().await {
            let _ = tokio_io::AsyncWriteExt::write_all(
                &mut buf_writer,
                &chunk.expect("Failed to get chunk"),
            )
            .await
            .context("Failed to write chunk.");
        }

        println!(
            r#"{} File "{}" downloaded successfully."#,
            "[INFO]".cyan().bold(),
            name
        );
    });
    
    join_all(dwn_threads).await;
    Ok(())
}
