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

class ChatsSearchUi extends Notifier<bool> {
  @override
  bool build() => false;

  void setOpen(bool open) => state = open;
}

final chatsSearchUiProvider = NotifierProvider<ChatsSearchUi, bool>(
  ChatsSearchUi.new,
);
