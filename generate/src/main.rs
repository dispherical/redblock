use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};

use csv::ReaderBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use ipnet::{Ipv4Net, Ipv6Net};
use iprange::IpRange;
use std::net::IpAddr;

fn ip_range_to_cidrs(start: &str, end: &str) -> Vec<String> {
    let s = match start.parse::<IpAddr>() {
        Ok(a) => a,
        Err(_) => return vec![],
    };
    let e = match end.parse::<IpAddr>() {
        Ok(a) => a,
        Err(_) => return vec![],
    };

    match (s, e) {
        (IpAddr::V4(sv4), IpAddr::V4(ev4)) => {
            let mut range: IpRange<Ipv4Net> = IpRange::new();

            let mut cur = u32::from(sv4);
            let end_u32 = u32::from(ev4);
            while cur <= end_u32 {
                let max_size = cur.trailing_zeros();
                let rem = 32 - (end_u32 - cur).leading_zeros();
                let prefix = std::cmp::max(32 - max_size, rem) as u8;
                let net = Ipv4Net::new(std::net::Ipv4Addr::from(cur), prefix).unwrap();
                range.add(net);
                let block_size = 1u128 << (32 - prefix) as u128;
                cur = (cur as u128 + block_size) as u32;
            }
            range.iter().map(|n| n.to_string()).collect()
        }
        (IpAddr::V6(sv6), IpAddr::V6(ev6)) => {
            use std::net::Ipv6Addr;
            let s_segments = u128::from(sv6);
            let e_segments = u128::from(ev6);
            if e_segments < s_segments {
                return vec![];
            }
            let count = e_segments - s_segments + 1;
            if count > 1024 {
                return vec![];
            }
            let mut range: IpRange<Ipv6Net> = IpRange::new();
            let mut cur = s_segments;
            while cur <= e_segments {
                let addr = Ipv6Addr::from(cur);
                let net = Ipv6Net::new(addr, 128).unwrap();
                range.add(net);
                cur += 1;
            }
            range.iter().map(|n| n.to_string()).collect()
        }
        _ => vec![],
    }
}

fn main() -> anyhow::Result<()> {
    let blocklist_states = vec![
        "Alabama",
        "Arkansas",
        "Florida",
        "Georgia",
        "Idaho",
        "Indiana",
        "Kansas",
        "Kentucky",
        "Louisiana",
        "Mississippi",
        "Missouri",
        "Montana",
        "Nebraska",
        "North Carolina",
        "North Dakota",
        "Ohio",
        "Oklahoma",
        "South Carolina",
        "South Dakota",
        "Tennessee",
        "Texas",
        "Utah",
        "Virginia",
        "Wyoming",
    ];

    let blocklist_countries = vec!["GB", "FR", "DE", "IT", "DK", "AU"];

    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_path("dbip-city-lite-2025-10.csv")?;

    let total_lines = rdr.records().count();
    let bar = ProgressBar::new(total_lines as u64);
    bar.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap(),
    );

    let mut regions: HashMap<String, HashSet<String>> = HashMap::new();
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_path("dbip-city-lite-2025-10.csv")?;

    for result in rdr.records() {
        let record = result?;
        bar.inc(1);

        let ip_start = &record[0];
        let ip_end = &record[1];
        let country = &record[3];
        let state = &record[4];

        let mut label = None;
        if country == "US" && blocklist_states.contains(&state) {
            label = Some(format!("US {}", state));
        } else if blocklist_countries.contains(&country) {
            label = Some(country.to_string());
        }

        if let Some(lbl) = label {
            let cidrs = ip_range_to_cidrs(ip_start, ip_end);
            let entry = regions.entry(lbl).or_default();
            for c in cidrs {
                entry.insert(c.to_string());
            }
        }
    }
    bar.finish();

    use chrono::Local;
    use ipnet::IpNet;
    let mut all_cidrs: Vec<(String, String)> = Vec::new();
    for (region, cidrs) in &regions {
        for c in cidrs {
            all_cidrs.push((region.clone(), c.clone()));
        }
    }

    let total_cidrs = all_cidrs.len();
    let mut total_ips: f64 = 0.0;
    for (_region, cidr) in &all_cidrs {
        if let Ok(net) = cidr.parse::<IpNet>() {
            let count = match net {
                IpNet::V4(v4) => {
                    let prefix = v4.prefix_len();
                    2f64.powi((32 - prefix) as i32)
                }
                IpNet::V6(v6) => {
                    let prefix = v6.prefix_len();
                    2f64.powi((128 - prefix) as i32)
                }
            };
            total_ips += count;
        }
    }

    let mut cidr_text = String::new();
    let mut last_region = "";
    for (region, cidr) in &all_cidrs {
        if region != last_region {
            cidr_text.push_str(&format!("# {}\n", region));
            last_region = region;
        }
        cidr_text.push_str(&format!("{}\n", cidr));
    }

    let now = Local::now();
    let timestamp = now.format("%a %b %d %Y %H:%M:%S GMT%z (%Z)").to_string();
    let size_bytes = (cidr_text.len()) as u64;
    let size_mb = (size_bytes as f64) / 1024.0 / 1024.0;

    let header = format!(
            "# A list of IP ranges (in CIDR format, newline-delimited) registered to U.S. states and countries that have enacted laws requiring age verification for online content deemed \"harmful to minors.\"\n# These laws are often written in vague or overly broad terms, and in practice, they have been used or proposed to restrict access to LGBTQ+ content, sexual health information, and other constitutionally protected material.\n# Updated {}\n# Total CIDR entries: {}\n# Total IPs blocked: {}\n# File size: {:.2} MB, {} bytes\n# Learn more at https://dispherical.com/tools/redblock/\n# Includes transformed data from DB-IP (db-ip.com) licensed under CC BY 4.0\n",
            timestamp,
            total_cidrs,
            total_ips as u64,
            size_mb,
            size_bytes
        );

    let mut header_file = BufWriter::new(File::create("header.txt")?);
    header_file.write_all(header.as_bytes())?;

    let mut output = BufWriter::new(File::create("list.txt")?);
    output.write_all(header.as_bytes())?;
    output.write_all(b"\n\n")?;
    output.write_all(cidr_text.as_bytes())?;

    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Read;
    let mut input = File::open("list.txt")?;
    let mut buffer = Vec::new();
    input.read_to_end(&mut buffer)?;
    let gz_file = File::create("list.txt.gz")?;
    let mut encoder = GzEncoder::new(gz_file, Compression::default());
    encoder.write_all(&buffer)?;
    encoder.finish()?;

    println!(
        "Done! Total CIDRs: {} | Total IPs: {} | File size: {:.2} MB",
        total_cidrs, total_ips, size_mb
    );
    Ok(())
}
