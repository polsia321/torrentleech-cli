use time::{OffsetDateTime, macros::datetime};
use torrentleech_cli::output::{render_search_compact, render_search_json};
use torrentleech_cli::{SearchResponse, SearchResult};
use url::Url;

fn result(id: u64, title: &str, added: &str) -> SearchResult {
    SearchResult {
        id,
        title: title.to_string(),
        category_id: Some(33),
        category: Some("0-day".to_string()),
        tags: vec!["FREELEECH".to_string()],
        added: Some(added.to_string()),
        comments: Some(1),
        size: Some("1 GiB".to_string()),
        completed: Some(2),
        seeders: Some(3),
        leechers: Some(4),
        uploader: None,
        download_url: Url::parse(&format!(
            "https://www.torrentleech.org/download/{id}/example.torrent"
        ))
        .unwrap(),
        freeleech: true,
    }
}

#[test]
fn compact_output_formats_age_thresholds() {
    let now: OffsetDateTime = datetime!(2026-05-29 12:00 UTC);
    let response = SearchResponse {
        query: Some("age".to_string()),
        page: 1,
        total: Some(6),
        results: vec![
            result(1, "minutes", "2026-05-29 11:01:00"),
            result(2, "hours", "2026-05-28 12:01:00"),
            result(3, "days", "2026-04-01 12:00:00"),
            result(4, "months", "2024-07-29 12:00:00"),
            result(5, "years", "2024-05-29 12:00:00"),
        ],
    };

    let output = render_search_compact(&response, now).unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert!(lines[0].contains("59m"));
    assert!(lines[1].contains("23h"));
    assert!(lines[2].contains("58d"));
    assert!(lines[3].contains("22mo"));
    assert!(lines[4].contains("2y"));
    assert_eq!(lines[0], "1 59m s3 l4 1 GiB 0-day FL minutes");
    assert!(!lines[0].contains("C:"));
    assert!(!lines[0].contains("FREELEECH"));
}

#[test]
fn zero_results_produce_empty_compact_output() {
    let response = SearchResponse {
        query: None,
        page: 1,
        total: Some(0),
        results: Vec::new(),
    };

    assert_eq!(
        render_search_compact(&response, datetime!(2026-05-29 12:00 UTC)).unwrap(),
        ""
    );
}

#[test]
fn json_output_uses_stable_search_response_shape() {
    let response = SearchResponse {
        query: Some("ubuntu".to_string()),
        page: 2,
        total: Some(1),
        results: vec![result(1, "ubuntu", "2026-05-29 11:01:00")],
    };

    let json = render_search_json(&response).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["query"], "ubuntu");
    assert_eq!(value["page"], 2);
    assert_eq!(value["total"], 1);
    assert_eq!(value["results"][0]["id"], 1);
    assert_eq!(value["results"][0]["title"], "ubuntu");
    assert_eq!(value["results"][0]["category_id"], 33);
    assert_eq!(value["results"][0]["category"], "0-day");
    assert_eq!(value["results"][0]["tags"][0], "FREELEECH");
    assert_eq!(value["results"][0]["added"], "2026-05-29 11:01:00");
    assert_eq!(value["results"][0]["comments"], 1);
    assert_eq!(value["results"][0]["size"], "1 GiB");
    assert_eq!(value["results"][0]["completed"], 2);
    assert_eq!(value["results"][0]["seeders"], 3);
    assert_eq!(value["results"][0]["leechers"], 4);
    assert_eq!(
        value["results"][0]["download_url"],
        "https://www.torrentleech.org/download/1/example.torrent"
    );
    assert!(value["results"][0].get("uploader").is_none());
    assert!(value["results"][0].get("freeleech").is_none());
}
