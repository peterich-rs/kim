import 'dart:convert';

String testJwt({required String acc, required int exp}) {
  String b64(String s) => base64Url.encode(utf8.encode(s)).replaceAll('=', '');
  return '${b64('{"alg":"none"}')}.${b64(jsonEncode({'acc': acc, 'exp': exp}))}.sig';
}
