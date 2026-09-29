/// Remote URL check for image display. Media kind is classified in Rust.
library;

bool isRemoteUrl(String body) {
  final url = body.trim();
  return url.startsWith('http://') || url.startsWith('https://');
}
