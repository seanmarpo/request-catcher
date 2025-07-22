# Request Catcher

A simple HTTP Request capture web application written in Rust, largely inspired by [request-baskets](https://github.com/darklynx/request-baskets). Useful for testing HTTP requests and responses such as webhooks, security testing for SSRF, and more.

You can interact with a demo instance of this application at:
- [https://request-catcher.fly.dev/](https://request-catcher.fly.dev/)

## Features
- User controlled, password protected buckets for capturing requests privately
- Ability to view all existing buckets making it easy to return to your space
- Ability to delete a bucket and/or clear all requests from a bucket
- Quick share your bucket link for collaboration
- And more planned, but not currently implemented

## Usage
- `cargo run` to start the application

### Docker Usage
- `docker build -t request-catcher .`
- `docker run -p 9090:9090 request-catcher`

## Feature Roadmap
- [ ] Better UI handling for requests as the UI gets cluttered quickly with multiple requests

## AI Disclaimer
Most of this application was "vibe-coded" as a personal exploration project. I do not personally know Rust, but I wanted to see what I could develop with the assistance of AI.
