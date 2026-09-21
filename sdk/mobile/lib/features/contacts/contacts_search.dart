library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

class ContactsSearchUi extends Notifier<bool> {
  @override
  bool build() => false;

  void setSearching(bool value) => state = value;
}

final contactsSearchUiProvider =
    NotifierProvider.autoDispose<ContactsSearchUi, bool>(ContactsSearchUi.new);
