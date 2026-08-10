use micro_http_server::{Client, MicroHTTP};
use orgize::ParseConfig;
use std::{
    borrow::Cow,
    fs::{File, read_to_string},
    io::Result,
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use crate::output::{
    accumulate, get_keywords,
    html::{Handler, PageHtml},
    infer_title,
};

pub fn serve(org_cfg: &ParseConfig, bindhost: SocketAddr) {
    let server = MicroHTTP::new(bindhost).unwrap();

    // we cannot print the actual listener address, MicroHTTP does
    // not expose that... no listening on port 0 i guess :(
    println!("listening on {bindhost}");

    while let Ok(Some(client)) = server.next_client() {
        _ = handle_request(client, org_cfg);
    }
}

fn handle_request(mut client: Client, org_cfg: &ParseConfig) -> Result<usize> {
    let Some(path) = client.request() else {
        return client.respond("400 Bad Request", b"why no request\n", &vec![]);
    };
    let path: Cow<[u8]> = percent_encoding::percent_decode_str(path).into();
    // prevent both accessing hidden files and path traversal
    if path.windows(2).any(|a| a == b"/." || a == b"\\.") {
        return client.respond("400 Bad Request", b"no bad\n", &vec![]);
    }
    // previous check may have missed stuff if the leading / is not there
    let Some(path) = path.strip_prefix(b"/") else {
        return client.respond("400 Bad Request", b"what the dog doin\n", &vec![]);
    };
    let mut pathb = PathBuf::from(
        #[cfg(unix)]
        std::ffi::OsStr::from_bytes(path),
        #[cfg(not(unix))]
        if let Ok(b) = str::from_utf8(path) {
            b
        } else {
            return client.respond(
                "400 Bad Request",
                b"non-utf-8 paths are only supported on unix\n",
                &vec![],
            );
        },
    );
    if pathb.has_root() {
        return client.respond("400 Bad Request", b"naughty\n", &vec![]);
    }
    if path.is_empty() || pathb.is_dir() {
        pathb.push("index.html");
    }
    if pathb.is_file() {
        let Ok((file, len)) = preview_static(&pathb) else {
            return client.respond(
                "500 Internal Service Error",
                b"thats a weird file\n",
                &vec![],
            );
        };
        let Ok(len) = len.try_into() else {
            return client.respond(
                "500 Internal Service Error",
                b"ur file is too big\n",
                &vec![],
            );
        };
        return client.respond_chunked(
            "200 OK",
            file,
            len,
            &if pathb
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("css"))
            {
                vec!["Content-Type: text/css".to_string()]
            } else {
                vec![]
            },
        );
    }
    if path == b"style.css" {
        return client.respond(
            "200 OK",
            crate::STYLESHEET,
            &vec!["Content-Type: text/css".to_string()],
        );
    }
    pathb.set_extension("org");
    if pathb.is_file() {
        let Some(preview) = preview_page(&pathb, org_cfg) else {
            return client.respond(
                "500 Internal Service Error",
                b"oh no org broke what did you do???\n",
                &vec![],
            );
        };
        return client.respond(
            "200 OK",
            preview.as_bytes(),
            &vec!["Content-Type: text/html".to_string()],
        );
    }

    client.respond("404 Not Found", b"how did i get here\n", &vec![])
}

fn preview_static(pathb: &PathBuf) -> Result<(File, u64)> {
    let file = File::open(pathb)?;
    let len = file.metadata()?.len();
    Ok((file, len))
}

fn preview_page(path: &Path, org_cfg: &ParseConfig) -> Option<String> {
    let fstr = read_to_string(path).ok()?;
    let res = org_cfg.clone().parse(fstr);

    let title = res.title().unwrap_or_else(|| infer_title(path));
    let keywords = get_keywords(&res);
    let accumulated = accumulate(&res);
    let lang = keywords.language.unwrap_or_else(|| "en".to_string());
    let numdir = path.iter().count();

    let mut html_export = Handler {
        numdir,
        accumulated,
        ..Default::default()
    };
    res.traverse(&mut html_export);

    let notice = Some(
        "you found my preview site. please avoid sharing the link around, don't be the reason this needs a more complex solution.",
    );

    let template = PageHtml {
        title: title.as_ref(),
        body: &html_export.exp.finish(),
        lang: &lang,
        numdir,
        notice,
        ..Default::default()
    };

    Some(template.to_string())
}
