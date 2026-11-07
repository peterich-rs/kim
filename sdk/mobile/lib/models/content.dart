library;

sealed class KimOutgoingContent {
  const KimOutgoingContent();

  const factory KimOutgoingContent.text(String text) = KimTextContent;

  const factory KimOutgoingContent.image({
    required String url,
    required int width,
    required int height,
  }) = KimImageContent;

  const factory KimOutgoingContent.video({required String url}) =
      KimVideoContent;
}

class KimTextContent extends KimOutgoingContent {
  const KimTextContent(this.text);

  final String text;
}

class KimImageContent extends KimOutgoingContent {
  const KimImageContent({
    required this.url,
    required this.width,
    required this.height,
  });

  final String url;
  final int width;
  final int height;
}

class KimVideoContent extends KimOutgoingContent {
  const KimVideoContent({required this.url});

  final String url;
}
