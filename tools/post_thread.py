# /// script
# requires-python = ">=3.11"
# dependencies = ["requests", "requests-oauthlib", "python-dotenv"]
# ///
"""Posta os cortes como respostas encadeadas numa thread do X (API v2, OAuth 1.0a)."""

import argparse
import os
import sys
import time
from pathlib import Path

import requests
from dotenv import load_dotenv
from requests_oauthlib import OAuth1

API = "https://api.x.com/2"
CHUNK = 4 * 1024 * 1024
ROOT = Path(__file__).resolve().parent.parent

POSTS = [
    ("clips/1_urna.mp4", "Dei vida à urna eletrônica. Ela flutua, mira e dispara. Sem recontagem. 🗳️🔴"),
    ("clips/2_wolverine.mp4", "No meio da briga eleitoral caiu um mutante do céu. SNIKT."),
    ("clips/3_lula.mp4", "Lula entrou na porrada. Não terminou bem."),
    ("clips/4_renan.mp4", "Renan Santos nocauteou o Lula. Foram 5 segundos de glória."),
    ("clips/5_flavio.mp4", "Flávio Bolsonaro vingou o Lula. Aí o Wolverine apareceu."),
]


def auth() -> OAuth1:
    load_dotenv(ROOT / ".env")
    keys = ["X_API_KEY", "X_API_SECRET", "X_ACCESS_TOKEN", "X_ACCESS_SECRET"]
    missing = [k for k in keys if not os.getenv(k)]
    if missing:
        sys.exit(f"Faltando no .env: {', '.join(missing)}")
    return OAuth1(*(os.environ[k] for k in keys))


def check(r: requests.Response) -> dict:
    if r.status_code >= 300:
        sys.exit(f"{r.request.method} {r.url} -> {r.status_code}\n{r.text}")
    return r.json() if r.content else {}


def upload_video(path: Path, oauth: OAuth1) -> str:
    size = path.stat().st_size
    init = check(requests.post(
        f"{API}/media/upload/initialize",
        json={"media_type": "video/mp4", "total_bytes": size, "media_category": "tweet_video"},
        auth=oauth,
    ))
    media_id = init["data"]["id"]
    with path.open("rb") as f:
        i = 0
        while chunk := f.read(CHUNK):
            check(requests.post(
                f"{API}/media/upload/{media_id}/append",
                data={"segment_index": str(i)},
                files={"media": chunk},
                auth=oauth,
            ))
            i += 1
    info = check(requests.post(f"{API}/media/upload/{media_id}/finalize", auth=oauth))["data"].get("processing_info")
    while info and info.get("state") in ("pending", "in_progress"):
        time.sleep(info.get("check_after_secs", 2))
        info = check(requests.get(
            f"{API}/media/upload",
            params={"command": "STATUS", "media_id": media_id},
            auth=oauth,
        ))["data"].get("processing_info")
    if info and info.get("state") == "failed":
        sys.exit(f"Processamento do vídeo falhou: {info}")
    return media_id


def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser()
    ap.add_argument("--reply-to", required=True, help="ID do último post da thread")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    if args.dry_run:
        for file, text in POSTS:
            print(f"[{(ROOT / file).stat().st_size / 1e6:.1f}MB] {file}: {text}")
        return

    oauth = auth()
    prev = args.reply_to
    for file, text in POSTS:
        media_id = upload_video(ROOT / file, oauth)
        post = check(requests.post(
            f"{API}/tweets",
            json={"text": text, "media": {"media_ids": [media_id]}, "reply": {"in_reply_to_tweet_id": prev}},
            auth=oauth,
        ))
        prev = post["data"]["id"]
        print(f"OK {file} -> https://x.com/i/status/{prev}")


if __name__ == "__main__":
    main()
