# Eule

A daily stock watchlist briefing that builds itself and costs nothing to run.

Every weekday at 17:15 New York time — an hour after the US close — a small Rust binary wakes up
in AWS Lambda, fetches quotes and headlines for a fixed list of tickers, asks an LLM for a
one-sentence explanation of each move, renders a single static HTML page and writes it to S3.
You read it from a bookmark on your phone.

Live example: <https://eule-briefing-viz.s3.eu-central-1.amazonaws.com/index.html>

```
EventBridge Scheduler (17:15 America/New_York, Mon-Fri)
    -> Lambda (Rust, arm64, ~10-20s)
        -> Finnhub   quotes, company news, earnings calendar
        -> Bedrock   one sentence per move, from the headlines only
        -> S3        index.html (today) + YYYY-MM-DD.html (archive)
```

Each row is a ticker, sorted by how much it moved, with the price, the percentage, the
explanation, the next earnings date, and links to the headlines the explanation was drawn from.
The links matter: the summary is generated text and can be wrong, so every claim stays one click
from its source.

## Why it is built this way

There is no server, no database, no frontend framework and no JavaScript on the page. The output
is one HTML file. That is the whole design — a static file is the cheapest thing to host, the
fastest thing to open on a phone with bad signal, and the least likely thing to break while
nobody is watching.

The Rust code is only the Lambda. Everything else — schedule, bucket, permissions — is
configuration.

## Running it yourself

You will need an AWS account, a free [Finnhub](https://finnhub.io) API key, Bedrock model access
in your region, and a Rust toolchain with [`cargo lambda`](https://cargo-lambda.info) and the AWS
CLI.

**1. Clone and pick your tickers.** Edit `watchlist.toml`:

```toml
tickers = ["AAPL", "MSFT", "NVDA"]
```

Note this file is pulled in with `include_str!`, so it is compiled into the binary. Changing the
list means rebuilding, not just editing.

**2. Point it at a model available in your region.** `src/llm.rs` has:

```rust
const MODEL: &str = "eu.anthropic.claude-haiku-4-5-20251001-v1:0";
```

The `eu.` prefix is an EU inference profile. In a US region you want `us.anthropic.…`. Check what
your region actually offers, and request access in the Bedrock console first — approval is not
always instant:

```
aws bedrock list-foundation-models --region <your-region>
```

**3. Run it locally.** Put `FINNHUB_KEY=…` in a `.env` file, then:

```
OUTPUT_TARGET=local cargo run
```

That writes `out/index.html` and touches no AWS except Bedrock. Open it in a browser. If the page
looks right, the rest is deployment.

**4. Deploy.**

```
aws s3 mb s3://your-bucket-name --region <your-region>
cargo lambda build --release --arm64
cargo lambda deploy
```

Then set the environment variables on the function:

```
aws lambda update-function-configuration --function-name Eule \
  --environment 'Variables={FINNHUB_KEY=...,OUTPUT_TARGET=s3,S3_BUCKET=your-bucket-name}'
```

**5. Give the function permission** to write to that bucket and call Bedrock — see
`role-policy.json`, attached to the role `cargo lambda deploy` created for you.

**6. Schedule it.** An EventBridge schedule with a role that may invoke the function
(`sched-policy.json`):

```
aws scheduler create-schedule --name eule-daily \
  --schedule-expression 'cron(15 17 ? * MON-FRI *)' \
  --schedule-expression-timezone America/New_York \
  --flexible-time-window '{"Mode":"OFF"}' \
  --target '{"Arn":"<function-arn>","RoleArn":"<scheduler-role-arn>","RetryPolicy":{"MaximumRetryAttempts":3}}'
```

The schedule is pinned to market time deliberately. 16:00 in New York is 21:00 or 22:00 in Berlin
depending on the month, because the US and EU daylight-saving windows do not line up — pinning to
`America/New_York` means never doing that arithmetic again.

## Configuration

| Variable | Required | Notes |
|---|---|---|
| `FINNHUB_KEY` | yes | Never in code or git. |
| `OUTPUT_TARGET` | no | `s3` writes to the bucket; anything else writes `out/index.html`. |
| `S3_BUCKET` | when `s3` | Target bucket. |

## Cost

Within the AWS free tier, and a few cents a month after it: one Lambda invocation per weekday,
one small-model Bedrock call per ticker per day, and a handful of S3 objects. There is nothing
always-on. Set a billing alarm anyway.

## Limits worth knowing

- **Finnhub's free tier is for personal, non-commercial use.** Use your own key, and do not
  republish or resell the page.
- **60 API calls per minute** on that tier. Eule makes three sequential calls per ticker, so
  around 18 tickers is the practical ceiling before requests start being refused.
- **The explanations are generated text.** They are constrained to the supplied headlines and
  "no clear driver" is an allowed answer, but they are not analysis and nothing here is advice.
- **The page is world-readable** once the bucket is public. It is a stock summary, so that is
  usually fine — but it is a decision, not a default to ignore.

## Development

```
cargo test     # unit tests, no network
cargo clippy
```


