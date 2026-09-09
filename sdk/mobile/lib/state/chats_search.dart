library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

class ChatsSearchTick extends Notifier<int> {
  @override
  int build() => 0;

  void request() => state++;
}

final chatsSearchTickProvider = NotifierProvider<ChatsSearchTick, int>(
  ChatsSearchTick.new,
);
