// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'types.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$LinkState {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkState);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LinkState()';
}


}

/// @nodoc
class $LinkStateCopyWith<$Res>  {
$LinkStateCopyWith(LinkState _, $Res Function(LinkState) __);
}


/// Adds pattern-matching-related methods to [LinkState].
extension LinkStatePatterns on LinkState {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( LinkState_Connecting value)?  connecting,TResult Function( LinkState_Online value)?  online,TResult Function( LinkState_Reconnecting value)?  reconnecting,TResult Function( LinkState_Offline value)?  offline,required TResult orElse(),}){
final _that = this;
switch (_that) {
case LinkState_Connecting() when connecting != null:
return connecting(_that);case LinkState_Online() when online != null:
return online(_that);case LinkState_Reconnecting() when reconnecting != null:
return reconnecting(_that);case LinkState_Offline() when offline != null:
return offline(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( LinkState_Connecting value)  connecting,required TResult Function( LinkState_Online value)  online,required TResult Function( LinkState_Reconnecting value)  reconnecting,required TResult Function( LinkState_Offline value)  offline,}){
final _that = this;
switch (_that) {
case LinkState_Connecting():
return connecting(_that);case LinkState_Online():
return online(_that);case LinkState_Reconnecting():
return reconnecting(_that);case LinkState_Offline():
return offline(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( LinkState_Connecting value)?  connecting,TResult? Function( LinkState_Online value)?  online,TResult? Function( LinkState_Reconnecting value)?  reconnecting,TResult? Function( LinkState_Offline value)?  offline,}){
final _that = this;
switch (_that) {
case LinkState_Connecting() when connecting != null:
return connecting(_that);case LinkState_Online() when online != null:
return online(_that);case LinkState_Reconnecting() when reconnecting != null:
return reconnecting(_that);case LinkState_Offline() when offline != null:
return offline(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  connecting,TResult Function()?  online,TResult Function( int attempt)?  reconnecting,TResult Function()?  offline,required TResult orElse(),}) {final _that = this;
switch (_that) {
case LinkState_Connecting() when connecting != null:
return connecting();case LinkState_Online() when online != null:
return online();case LinkState_Reconnecting() when reconnecting != null:
return reconnecting(_that.attempt);case LinkState_Offline() when offline != null:
return offline();case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  connecting,required TResult Function()  online,required TResult Function( int attempt)  reconnecting,required TResult Function()  offline,}) {final _that = this;
switch (_that) {
case LinkState_Connecting():
return connecting();case LinkState_Online():
return online();case LinkState_Reconnecting():
return reconnecting(_that.attempt);case LinkState_Offline():
return offline();}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  connecting,TResult? Function()?  online,TResult? Function( int attempt)?  reconnecting,TResult? Function()?  offline,}) {final _that = this;
switch (_that) {
case LinkState_Connecting() when connecting != null:
return connecting();case LinkState_Online() when online != null:
return online();case LinkState_Reconnecting() when reconnecting != null:
return reconnecting(_that.attempt);case LinkState_Offline() when offline != null:
return offline();case _:
  return null;

}
}

}

/// @nodoc


class LinkState_Connecting extends LinkState {
  const LinkState_Connecting(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkState_Connecting);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LinkState.connecting()';
}


}




/// @nodoc


class LinkState_Online extends LinkState {
  const LinkState_Online(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkState_Online);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LinkState.online()';
}


}




/// @nodoc


class LinkState_Reconnecting extends LinkState {
  const LinkState_Reconnecting({required this.attempt}): super._();
  

 final  int attempt;

/// Create a copy of LinkState
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LinkState_ReconnectingCopyWith<LinkState_Reconnecting> get copyWith => _$LinkState_ReconnectingCopyWithImpl<LinkState_Reconnecting>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkState_Reconnecting&&(identical(other.attempt, attempt) || other.attempt == attempt));
}


@override
int get hashCode => Object.hash(runtimeType,attempt);

@override
String toString() {
  return 'LinkState.reconnecting(attempt: $attempt)';
}


}

/// @nodoc
abstract mixin class $LinkState_ReconnectingCopyWith<$Res> implements $LinkStateCopyWith<$Res> {
  factory $LinkState_ReconnectingCopyWith(LinkState_Reconnecting value, $Res Function(LinkState_Reconnecting) _then) = _$LinkState_ReconnectingCopyWithImpl;
@useResult
$Res call({
 int attempt
});




}
/// @nodoc
class _$LinkState_ReconnectingCopyWithImpl<$Res>
    implements $LinkState_ReconnectingCopyWith<$Res> {
  _$LinkState_ReconnectingCopyWithImpl(this._self, this._then);

  final LinkState_Reconnecting _self;
  final $Res Function(LinkState_Reconnecting) _then;

/// Create a copy of LinkState
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? attempt = null,}) {
  return _then(LinkState_Reconnecting(
attempt: null == attempt ? _self.attempt : attempt // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class LinkState_Offline extends LinkState {
  const LinkState_Offline(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkState_Offline);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LinkState.offline()';
}


}




/// @nodoc
mixin _$SessionUpdate {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SessionUpdate()';
}


}

/// @nodoc
class $SessionUpdateCopyWith<$Res>  {
$SessionUpdateCopyWith(SessionUpdate _, $Res Function(SessionUpdate) __);
}


/// Adds pattern-matching-related methods to [SessionUpdate].
extension SessionUpdatePatterns on SessionUpdate {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SessionUpdate_Link value)?  link,TResult Function( SessionUpdate_Inbox value)?  inbox,TResult Function( SessionUpdate_ThreadUpsert value)?  threadUpsert,TResult Function( SessionUpdate_SyncProgress value)?  syncProgress,TResult Function( SessionUpdate_Kickout value)?  kickout,TResult Function( SessionUpdate_AuthExpired value)?  authExpired,TResult Function( SessionUpdate_TokenRenew value)?  tokenRenew,TResult Function( SessionUpdate_FriendRequest value)?  friendRequest,TResult Function( SessionUpdate_FriendAccepted value)?  friendAccepted,TResult Function( SessionUpdate_ProfileUpdated value)?  profileUpdated,TResult Function( SessionUpdate_Presence value)?  presence,TResult Function( SessionUpdate_Typing value)?  typing,TResult Function( SessionUpdate_ReceiptRead value)?  receiptRead,TResult Function( SessionUpdate_GroupCreate value)?  groupCreate,TResult Function( SessionUpdate_ContactsChanged value)?  contactsChanged,TResult Function( SessionUpdate_AgentTurn value)?  agentTurn,TResult Function( SessionUpdate_AgentCard value)?  agentCard,TResult Function( SessionUpdate_RustPanic value)?  rustPanic,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SessionUpdate_Link() when link != null:
return link(_that);case SessionUpdate_Inbox() when inbox != null:
return inbox(_that);case SessionUpdate_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that);case SessionUpdate_SyncProgress() when syncProgress != null:
return syncProgress(_that);case SessionUpdate_Kickout() when kickout != null:
return kickout(_that);case SessionUpdate_AuthExpired() when authExpired != null:
return authExpired(_that);case SessionUpdate_TokenRenew() when tokenRenew != null:
return tokenRenew(_that);case SessionUpdate_FriendRequest() when friendRequest != null:
return friendRequest(_that);case SessionUpdate_FriendAccepted() when friendAccepted != null:
return friendAccepted(_that);case SessionUpdate_ProfileUpdated() when profileUpdated != null:
return profileUpdated(_that);case SessionUpdate_Presence() when presence != null:
return presence(_that);case SessionUpdate_Typing() when typing != null:
return typing(_that);case SessionUpdate_ReceiptRead() when receiptRead != null:
return receiptRead(_that);case SessionUpdate_GroupCreate() when groupCreate != null:
return groupCreate(_that);case SessionUpdate_ContactsChanged() when contactsChanged != null:
return contactsChanged(_that);case SessionUpdate_AgentTurn() when agentTurn != null:
return agentTurn(_that);case SessionUpdate_AgentCard() when agentCard != null:
return agentCard(_that);case SessionUpdate_RustPanic() when rustPanic != null:
return rustPanic(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SessionUpdate_Link value)  link,required TResult Function( SessionUpdate_Inbox value)  inbox,required TResult Function( SessionUpdate_ThreadUpsert value)  threadUpsert,required TResult Function( SessionUpdate_SyncProgress value)  syncProgress,required TResult Function( SessionUpdate_Kickout value)  kickout,required TResult Function( SessionUpdate_AuthExpired value)  authExpired,required TResult Function( SessionUpdate_TokenRenew value)  tokenRenew,required TResult Function( SessionUpdate_FriendRequest value)  friendRequest,required TResult Function( SessionUpdate_FriendAccepted value)  friendAccepted,required TResult Function( SessionUpdate_ProfileUpdated value)  profileUpdated,required TResult Function( SessionUpdate_Presence value)  presence,required TResult Function( SessionUpdate_Typing value)  typing,required TResult Function( SessionUpdate_ReceiptRead value)  receiptRead,required TResult Function( SessionUpdate_GroupCreate value)  groupCreate,required TResult Function( SessionUpdate_ContactsChanged value)  contactsChanged,required TResult Function( SessionUpdate_AgentTurn value)  agentTurn,required TResult Function( SessionUpdate_AgentCard value)  agentCard,required TResult Function( SessionUpdate_RustPanic value)  rustPanic,}){
final _that = this;
switch (_that) {
case SessionUpdate_Link():
return link(_that);case SessionUpdate_Inbox():
return inbox(_that);case SessionUpdate_ThreadUpsert():
return threadUpsert(_that);case SessionUpdate_SyncProgress():
return syncProgress(_that);case SessionUpdate_Kickout():
return kickout(_that);case SessionUpdate_AuthExpired():
return authExpired(_that);case SessionUpdate_TokenRenew():
return tokenRenew(_that);case SessionUpdate_FriendRequest():
return friendRequest(_that);case SessionUpdate_FriendAccepted():
return friendAccepted(_that);case SessionUpdate_ProfileUpdated():
return profileUpdated(_that);case SessionUpdate_Presence():
return presence(_that);case SessionUpdate_Typing():
return typing(_that);case SessionUpdate_ReceiptRead():
return receiptRead(_that);case SessionUpdate_GroupCreate():
return groupCreate(_that);case SessionUpdate_ContactsChanged():
return contactsChanged(_that);case SessionUpdate_AgentTurn():
return agentTurn(_that);case SessionUpdate_AgentCard():
return agentCard(_that);case SessionUpdate_RustPanic():
return rustPanic(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SessionUpdate_Link value)?  link,TResult? Function( SessionUpdate_Inbox value)?  inbox,TResult? Function( SessionUpdate_ThreadUpsert value)?  threadUpsert,TResult? Function( SessionUpdate_SyncProgress value)?  syncProgress,TResult? Function( SessionUpdate_Kickout value)?  kickout,TResult? Function( SessionUpdate_AuthExpired value)?  authExpired,TResult? Function( SessionUpdate_TokenRenew value)?  tokenRenew,TResult? Function( SessionUpdate_FriendRequest value)?  friendRequest,TResult? Function( SessionUpdate_FriendAccepted value)?  friendAccepted,TResult? Function( SessionUpdate_ProfileUpdated value)?  profileUpdated,TResult? Function( SessionUpdate_Presence value)?  presence,TResult? Function( SessionUpdate_Typing value)?  typing,TResult? Function( SessionUpdate_ReceiptRead value)?  receiptRead,TResult? Function( SessionUpdate_GroupCreate value)?  groupCreate,TResult? Function( SessionUpdate_ContactsChanged value)?  contactsChanged,TResult? Function( SessionUpdate_AgentTurn value)?  agentTurn,TResult? Function( SessionUpdate_AgentCard value)?  agentCard,TResult? Function( SessionUpdate_RustPanic value)?  rustPanic,}){
final _that = this;
switch (_that) {
case SessionUpdate_Link() when link != null:
return link(_that);case SessionUpdate_Inbox() when inbox != null:
return inbox(_that);case SessionUpdate_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that);case SessionUpdate_SyncProgress() when syncProgress != null:
return syncProgress(_that);case SessionUpdate_Kickout() when kickout != null:
return kickout(_that);case SessionUpdate_AuthExpired() when authExpired != null:
return authExpired(_that);case SessionUpdate_TokenRenew() when tokenRenew != null:
return tokenRenew(_that);case SessionUpdate_FriendRequest() when friendRequest != null:
return friendRequest(_that);case SessionUpdate_FriendAccepted() when friendAccepted != null:
return friendAccepted(_that);case SessionUpdate_ProfileUpdated() when profileUpdated != null:
return profileUpdated(_that);case SessionUpdate_Presence() when presence != null:
return presence(_that);case SessionUpdate_Typing() when typing != null:
return typing(_that);case SessionUpdate_ReceiptRead() when receiptRead != null:
return receiptRead(_that);case SessionUpdate_GroupCreate() when groupCreate != null:
return groupCreate(_that);case SessionUpdate_ContactsChanged() when contactsChanged != null:
return contactsChanged(_that);case SessionUpdate_AgentTurn() when agentTurn != null:
return agentTurn(_that);case SessionUpdate_AgentCard() when agentCard != null:
return agentCard(_that);case SessionUpdate_RustPanic() when rustPanic != null:
return rustPanic(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( LinkState state,  String? lastError)?  link,TResult Function( List<ThreadView> threads)?  inbox,TResult Function( ThreadView thread)?  threadUpsert,TResult Function( BigInt pulled,  bool catchingUp)?  syncProgress,TResult Function( String channelId)?  kickout,TResult Function( String reason)?  authExpired,TResult Function( String token,  PlatformInt64 exp)?  tokenRenew,TResult Function( String from,  String nickname)?  friendRequest,TResult Function( String from,  String nickname)?  friendAccepted,TResult Function( String account,  String nickname,  String avatar)?  profileUpdated,TResult Function( String account,  int status,  PlatformInt64 lastSeen)?  presence,TResult Function( String typer,  String dest,  int kind,  bool active)?  typing,TResult Function( String reader,  String dest,  int kind,  PlatformInt64 messageId)?  receiptRead,TResult Function( String groupId,  List<String> members)?  groupCreate,TResult Function( List<Person> contacts)?  contactsChanged,TResult Function( String dest,  AgentTurnState state,  String text)?  agentTurn,TResult Function( String dest,  AgentCard card)?  agentCard,TResult Function( String message)?  rustPanic,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SessionUpdate_Link() when link != null:
return link(_that.state,_that.lastError);case SessionUpdate_Inbox() when inbox != null:
return inbox(_that.threads);case SessionUpdate_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that.thread);case SessionUpdate_SyncProgress() when syncProgress != null:
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdate_Kickout() when kickout != null:
return kickout(_that.channelId);case SessionUpdate_AuthExpired() when authExpired != null:
return authExpired(_that.reason);case SessionUpdate_TokenRenew() when tokenRenew != null:
return tokenRenew(_that.token,_that.exp);case SessionUpdate_FriendRequest() when friendRequest != null:
return friendRequest(_that.from,_that.nickname);case SessionUpdate_FriendAccepted() when friendAccepted != null:
return friendAccepted(_that.from,_that.nickname);case SessionUpdate_ProfileUpdated() when profileUpdated != null:
return profileUpdated(_that.account,_that.nickname,_that.avatar);case SessionUpdate_Presence() when presence != null:
return presence(_that.account,_that.status,_that.lastSeen);case SessionUpdate_Typing() when typing != null:
return typing(_that.typer,_that.dest,_that.kind,_that.active);case SessionUpdate_ReceiptRead() when receiptRead != null:
return receiptRead(_that.reader,_that.dest,_that.kind,_that.messageId);case SessionUpdate_GroupCreate() when groupCreate != null:
return groupCreate(_that.groupId,_that.members);case SessionUpdate_ContactsChanged() when contactsChanged != null:
return contactsChanged(_that.contacts);case SessionUpdate_AgentTurn() when agentTurn != null:
return agentTurn(_that.dest,_that.state,_that.text);case SessionUpdate_AgentCard() when agentCard != null:
return agentCard(_that.dest,_that.card);case SessionUpdate_RustPanic() when rustPanic != null:
return rustPanic(_that.message);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( LinkState state,  String? lastError)  link,required TResult Function( List<ThreadView> threads)  inbox,required TResult Function( ThreadView thread)  threadUpsert,required TResult Function( BigInt pulled,  bool catchingUp)  syncProgress,required TResult Function( String channelId)  kickout,required TResult Function( String reason)  authExpired,required TResult Function( String token,  PlatformInt64 exp)  tokenRenew,required TResult Function( String from,  String nickname)  friendRequest,required TResult Function( String from,  String nickname)  friendAccepted,required TResult Function( String account,  String nickname,  String avatar)  profileUpdated,required TResult Function( String account,  int status,  PlatformInt64 lastSeen)  presence,required TResult Function( String typer,  String dest,  int kind,  bool active)  typing,required TResult Function( String reader,  String dest,  int kind,  PlatformInt64 messageId)  receiptRead,required TResult Function( String groupId,  List<String> members)  groupCreate,required TResult Function( List<Person> contacts)  contactsChanged,required TResult Function( String dest,  AgentTurnState state,  String text)  agentTurn,required TResult Function( String dest,  AgentCard card)  agentCard,required TResult Function( String message)  rustPanic,}) {final _that = this;
switch (_that) {
case SessionUpdate_Link():
return link(_that.state,_that.lastError);case SessionUpdate_Inbox():
return inbox(_that.threads);case SessionUpdate_ThreadUpsert():
return threadUpsert(_that.thread);case SessionUpdate_SyncProgress():
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdate_Kickout():
return kickout(_that.channelId);case SessionUpdate_AuthExpired():
return authExpired(_that.reason);case SessionUpdate_TokenRenew():
return tokenRenew(_that.token,_that.exp);case SessionUpdate_FriendRequest():
return friendRequest(_that.from,_that.nickname);case SessionUpdate_FriendAccepted():
return friendAccepted(_that.from,_that.nickname);case SessionUpdate_ProfileUpdated():
return profileUpdated(_that.account,_that.nickname,_that.avatar);case SessionUpdate_Presence():
return presence(_that.account,_that.status,_that.lastSeen);case SessionUpdate_Typing():
return typing(_that.typer,_that.dest,_that.kind,_that.active);case SessionUpdate_ReceiptRead():
return receiptRead(_that.reader,_that.dest,_that.kind,_that.messageId);case SessionUpdate_GroupCreate():
return groupCreate(_that.groupId,_that.members);case SessionUpdate_ContactsChanged():
return contactsChanged(_that.contacts);case SessionUpdate_AgentTurn():
return agentTurn(_that.dest,_that.state,_that.text);case SessionUpdate_AgentCard():
return agentCard(_that.dest,_that.card);case SessionUpdate_RustPanic():
return rustPanic(_that.message);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( LinkState state,  String? lastError)?  link,TResult? Function( List<ThreadView> threads)?  inbox,TResult? Function( ThreadView thread)?  threadUpsert,TResult? Function( BigInt pulled,  bool catchingUp)?  syncProgress,TResult? Function( String channelId)?  kickout,TResult? Function( String reason)?  authExpired,TResult? Function( String token,  PlatformInt64 exp)?  tokenRenew,TResult? Function( String from,  String nickname)?  friendRequest,TResult? Function( String from,  String nickname)?  friendAccepted,TResult? Function( String account,  String nickname,  String avatar)?  profileUpdated,TResult? Function( String account,  int status,  PlatformInt64 lastSeen)?  presence,TResult? Function( String typer,  String dest,  int kind,  bool active)?  typing,TResult? Function( String reader,  String dest,  int kind,  PlatformInt64 messageId)?  receiptRead,TResult? Function( String groupId,  List<String> members)?  groupCreate,TResult? Function( List<Person> contacts)?  contactsChanged,TResult? Function( String dest,  AgentTurnState state,  String text)?  agentTurn,TResult? Function( String dest,  AgentCard card)?  agentCard,TResult? Function( String message)?  rustPanic,}) {final _that = this;
switch (_that) {
case SessionUpdate_Link() when link != null:
return link(_that.state,_that.lastError);case SessionUpdate_Inbox() when inbox != null:
return inbox(_that.threads);case SessionUpdate_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that.thread);case SessionUpdate_SyncProgress() when syncProgress != null:
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdate_Kickout() when kickout != null:
return kickout(_that.channelId);case SessionUpdate_AuthExpired() when authExpired != null:
return authExpired(_that.reason);case SessionUpdate_TokenRenew() when tokenRenew != null:
return tokenRenew(_that.token,_that.exp);case SessionUpdate_FriendRequest() when friendRequest != null:
return friendRequest(_that.from,_that.nickname);case SessionUpdate_FriendAccepted() when friendAccepted != null:
return friendAccepted(_that.from,_that.nickname);case SessionUpdate_ProfileUpdated() when profileUpdated != null:
return profileUpdated(_that.account,_that.nickname,_that.avatar);case SessionUpdate_Presence() when presence != null:
return presence(_that.account,_that.status,_that.lastSeen);case SessionUpdate_Typing() when typing != null:
return typing(_that.typer,_that.dest,_that.kind,_that.active);case SessionUpdate_ReceiptRead() when receiptRead != null:
return receiptRead(_that.reader,_that.dest,_that.kind,_that.messageId);case SessionUpdate_GroupCreate() when groupCreate != null:
return groupCreate(_that.groupId,_that.members);case SessionUpdate_ContactsChanged() when contactsChanged != null:
return contactsChanged(_that.contacts);case SessionUpdate_AgentTurn() when agentTurn != null:
return agentTurn(_that.dest,_that.state,_that.text);case SessionUpdate_AgentCard() when agentCard != null:
return agentCard(_that.dest,_that.card);case SessionUpdate_RustPanic() when rustPanic != null:
return rustPanic(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class SessionUpdate_Link extends SessionUpdate {
  const SessionUpdate_Link({required this.state, this.lastError}): super._();
  

 final  LinkState state;
 final  String? lastError;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_LinkCopyWith<SessionUpdate_Link> get copyWith => _$SessionUpdate_LinkCopyWithImpl<SessionUpdate_Link>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_Link&&(identical(other.state, state) || other.state == state)&&(identical(other.lastError, lastError) || other.lastError == lastError));
}


@override
int get hashCode => Object.hash(runtimeType,state,lastError);

@override
String toString() {
  return 'SessionUpdate.link(state: $state, lastError: $lastError)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_LinkCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_LinkCopyWith(SessionUpdate_Link value, $Res Function(SessionUpdate_Link) _then) = _$SessionUpdate_LinkCopyWithImpl;
@useResult
$Res call({
 LinkState state, String? lastError
});


$LinkStateCopyWith<$Res> get state;

}
/// @nodoc
class _$SessionUpdate_LinkCopyWithImpl<$Res>
    implements $SessionUpdate_LinkCopyWith<$Res> {
  _$SessionUpdate_LinkCopyWithImpl(this._self, this._then);

  final SessionUpdate_Link _self;
  final $Res Function(SessionUpdate_Link) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? state = null,Object? lastError = freezed,}) {
  return _then(SessionUpdate_Link(
state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as LinkState,lastError: freezed == lastError ? _self.lastError : lastError // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@override
@pragma('vm:prefer-inline')
$LinkStateCopyWith<$Res> get state {
  
  return $LinkStateCopyWith<$Res>(_self.state, (value) {
    return _then(_self.copyWith(state: value));
  });
}
}

/// @nodoc


class SessionUpdate_Inbox extends SessionUpdate {
  const SessionUpdate_Inbox({required  List<ThreadView> threads}): _threads = threads,super._();
  

 final  List<ThreadView> _threads;
 List<ThreadView> get threads {
  if (_threads is EqualUnmodifiableListView) return _threads;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_threads);
}


/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_InboxCopyWith<SessionUpdate_Inbox> get copyWith => _$SessionUpdate_InboxCopyWithImpl<SessionUpdate_Inbox>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_Inbox&&const DeepCollectionEquality().equals(other._threads, _threads));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_threads));

@override
String toString() {
  return 'SessionUpdate.inbox(threads: $threads)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_InboxCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_InboxCopyWith(SessionUpdate_Inbox value, $Res Function(SessionUpdate_Inbox) _then) = _$SessionUpdate_InboxCopyWithImpl;
@useResult
$Res call({
 List<ThreadView> threads
});




}
/// @nodoc
class _$SessionUpdate_InboxCopyWithImpl<$Res>
    implements $SessionUpdate_InboxCopyWith<$Res> {
  _$SessionUpdate_InboxCopyWithImpl(this._self, this._then);

  final SessionUpdate_Inbox _self;
  final $Res Function(SessionUpdate_Inbox) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? threads = null,}) {
  return _then(SessionUpdate_Inbox(
threads: null == threads ? _self._threads : threads // ignore: cast_nullable_to_non_nullable
as List<ThreadView>,
  ));
}


}

/// @nodoc


class SessionUpdate_ThreadUpsert extends SessionUpdate {
  const SessionUpdate_ThreadUpsert({required this.thread}): super._();
  

 final  ThreadView thread;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_ThreadUpsertCopyWith<SessionUpdate_ThreadUpsert> get copyWith => _$SessionUpdate_ThreadUpsertCopyWithImpl<SessionUpdate_ThreadUpsert>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_ThreadUpsert&&(identical(other.thread, thread) || other.thread == thread));
}


@override
int get hashCode => Object.hash(runtimeType,thread);

@override
String toString() {
  return 'SessionUpdate.threadUpsert(thread: $thread)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_ThreadUpsertCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_ThreadUpsertCopyWith(SessionUpdate_ThreadUpsert value, $Res Function(SessionUpdate_ThreadUpsert) _then) = _$SessionUpdate_ThreadUpsertCopyWithImpl;
@useResult
$Res call({
 ThreadView thread
});




}
/// @nodoc
class _$SessionUpdate_ThreadUpsertCopyWithImpl<$Res>
    implements $SessionUpdate_ThreadUpsertCopyWith<$Res> {
  _$SessionUpdate_ThreadUpsertCopyWithImpl(this._self, this._then);

  final SessionUpdate_ThreadUpsert _self;
  final $Res Function(SessionUpdate_ThreadUpsert) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? thread = null,}) {
  return _then(SessionUpdate_ThreadUpsert(
thread: null == thread ? _self.thread : thread // ignore: cast_nullable_to_non_nullable
as ThreadView,
  ));
}


}

/// @nodoc


class SessionUpdate_SyncProgress extends SessionUpdate {
  const SessionUpdate_SyncProgress({required this.pulled, required this.catchingUp}): super._();
  

 final  BigInt pulled;
 final  bool catchingUp;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_SyncProgressCopyWith<SessionUpdate_SyncProgress> get copyWith => _$SessionUpdate_SyncProgressCopyWithImpl<SessionUpdate_SyncProgress>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_SyncProgress&&(identical(other.pulled, pulled) || other.pulled == pulled)&&(identical(other.catchingUp, catchingUp) || other.catchingUp == catchingUp));
}


@override
int get hashCode => Object.hash(runtimeType,pulled,catchingUp);

@override
String toString() {
  return 'SessionUpdate.syncProgress(pulled: $pulled, catchingUp: $catchingUp)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_SyncProgressCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_SyncProgressCopyWith(SessionUpdate_SyncProgress value, $Res Function(SessionUpdate_SyncProgress) _then) = _$SessionUpdate_SyncProgressCopyWithImpl;
@useResult
$Res call({
 BigInt pulled, bool catchingUp
});




}
/// @nodoc
class _$SessionUpdate_SyncProgressCopyWithImpl<$Res>
    implements $SessionUpdate_SyncProgressCopyWith<$Res> {
  _$SessionUpdate_SyncProgressCopyWithImpl(this._self, this._then);

  final SessionUpdate_SyncProgress _self;
  final $Res Function(SessionUpdate_SyncProgress) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? pulled = null,Object? catchingUp = null,}) {
  return _then(SessionUpdate_SyncProgress(
pulled: null == pulled ? _self.pulled : pulled // ignore: cast_nullable_to_non_nullable
as BigInt,catchingUp: null == catchingUp ? _self.catchingUp : catchingUp // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class SessionUpdate_Kickout extends SessionUpdate {
  const SessionUpdate_Kickout({required this.channelId}): super._();
  

 final  String channelId;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_KickoutCopyWith<SessionUpdate_Kickout> get copyWith => _$SessionUpdate_KickoutCopyWithImpl<SessionUpdate_Kickout>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_Kickout&&(identical(other.channelId, channelId) || other.channelId == channelId));
}


@override
int get hashCode => Object.hash(runtimeType,channelId);

@override
String toString() {
  return 'SessionUpdate.kickout(channelId: $channelId)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_KickoutCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_KickoutCopyWith(SessionUpdate_Kickout value, $Res Function(SessionUpdate_Kickout) _then) = _$SessionUpdate_KickoutCopyWithImpl;
@useResult
$Res call({
 String channelId
});




}
/// @nodoc
class _$SessionUpdate_KickoutCopyWithImpl<$Res>
    implements $SessionUpdate_KickoutCopyWith<$Res> {
  _$SessionUpdate_KickoutCopyWithImpl(this._self, this._then);

  final SessionUpdate_Kickout _self;
  final $Res Function(SessionUpdate_Kickout) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? channelId = null,}) {
  return _then(SessionUpdate_Kickout(
channelId: null == channelId ? _self.channelId : channelId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdate_AuthExpired extends SessionUpdate {
  const SessionUpdate_AuthExpired({required this.reason}): super._();
  

 final  String reason;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_AuthExpiredCopyWith<SessionUpdate_AuthExpired> get copyWith => _$SessionUpdate_AuthExpiredCopyWithImpl<SessionUpdate_AuthExpired>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_AuthExpired&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,reason);

@override
String toString() {
  return 'SessionUpdate.authExpired(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_AuthExpiredCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_AuthExpiredCopyWith(SessionUpdate_AuthExpired value, $Res Function(SessionUpdate_AuthExpired) _then) = _$SessionUpdate_AuthExpiredCopyWithImpl;
@useResult
$Res call({
 String reason
});




}
/// @nodoc
class _$SessionUpdate_AuthExpiredCopyWithImpl<$Res>
    implements $SessionUpdate_AuthExpiredCopyWith<$Res> {
  _$SessionUpdate_AuthExpiredCopyWithImpl(this._self, this._then);

  final SessionUpdate_AuthExpired _self;
  final $Res Function(SessionUpdate_AuthExpired) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(SessionUpdate_AuthExpired(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdate_TokenRenew extends SessionUpdate {
  const SessionUpdate_TokenRenew({required this.token, required this.exp}): super._();
  

 final  String token;
 final  PlatformInt64 exp;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_TokenRenewCopyWith<SessionUpdate_TokenRenew> get copyWith => _$SessionUpdate_TokenRenewCopyWithImpl<SessionUpdate_TokenRenew>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_TokenRenew&&(identical(other.token, token) || other.token == token)&&(identical(other.exp, exp) || other.exp == exp));
}


@override
int get hashCode => Object.hash(runtimeType,token,exp);

@override
String toString() {
  return 'SessionUpdate.tokenRenew(token: $token, exp: $exp)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_TokenRenewCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_TokenRenewCopyWith(SessionUpdate_TokenRenew value, $Res Function(SessionUpdate_TokenRenew) _then) = _$SessionUpdate_TokenRenewCopyWithImpl;
@useResult
$Res call({
 String token, PlatformInt64 exp
});




}
/// @nodoc
class _$SessionUpdate_TokenRenewCopyWithImpl<$Res>
    implements $SessionUpdate_TokenRenewCopyWith<$Res> {
  _$SessionUpdate_TokenRenewCopyWithImpl(this._self, this._then);

  final SessionUpdate_TokenRenew _self;
  final $Res Function(SessionUpdate_TokenRenew) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? token = null,Object? exp = null,}) {
  return _then(SessionUpdate_TokenRenew(
token: null == token ? _self.token : token // ignore: cast_nullable_to_non_nullable
as String,exp: null == exp ? _self.exp : exp // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class SessionUpdate_FriendRequest extends SessionUpdate {
  const SessionUpdate_FriendRequest({required this.from, required this.nickname}): super._();
  

 final  String from;
 final  String nickname;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_FriendRequestCopyWith<SessionUpdate_FriendRequest> get copyWith => _$SessionUpdate_FriendRequestCopyWithImpl<SessionUpdate_FriendRequest>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_FriendRequest&&(identical(other.from, from) || other.from == from)&&(identical(other.nickname, nickname) || other.nickname == nickname));
}


@override
int get hashCode => Object.hash(runtimeType,from,nickname);

@override
String toString() {
  return 'SessionUpdate.friendRequest(from: $from, nickname: $nickname)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_FriendRequestCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_FriendRequestCopyWith(SessionUpdate_FriendRequest value, $Res Function(SessionUpdate_FriendRequest) _then) = _$SessionUpdate_FriendRequestCopyWithImpl;
@useResult
$Res call({
 String from, String nickname
});




}
/// @nodoc
class _$SessionUpdate_FriendRequestCopyWithImpl<$Res>
    implements $SessionUpdate_FriendRequestCopyWith<$Res> {
  _$SessionUpdate_FriendRequestCopyWithImpl(this._self, this._then);

  final SessionUpdate_FriendRequest _self;
  final $Res Function(SessionUpdate_FriendRequest) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? from = null,Object? nickname = null,}) {
  return _then(SessionUpdate_FriendRequest(
from: null == from ? _self.from : from // ignore: cast_nullable_to_non_nullable
as String,nickname: null == nickname ? _self.nickname : nickname // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdate_FriendAccepted extends SessionUpdate {
  const SessionUpdate_FriendAccepted({required this.from, required this.nickname}): super._();
  

 final  String from;
 final  String nickname;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_FriendAcceptedCopyWith<SessionUpdate_FriendAccepted> get copyWith => _$SessionUpdate_FriendAcceptedCopyWithImpl<SessionUpdate_FriendAccepted>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_FriendAccepted&&(identical(other.from, from) || other.from == from)&&(identical(other.nickname, nickname) || other.nickname == nickname));
}


@override
int get hashCode => Object.hash(runtimeType,from,nickname);

@override
String toString() {
  return 'SessionUpdate.friendAccepted(from: $from, nickname: $nickname)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_FriendAcceptedCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_FriendAcceptedCopyWith(SessionUpdate_FriendAccepted value, $Res Function(SessionUpdate_FriendAccepted) _then) = _$SessionUpdate_FriendAcceptedCopyWithImpl;
@useResult
$Res call({
 String from, String nickname
});




}
/// @nodoc
class _$SessionUpdate_FriendAcceptedCopyWithImpl<$Res>
    implements $SessionUpdate_FriendAcceptedCopyWith<$Res> {
  _$SessionUpdate_FriendAcceptedCopyWithImpl(this._self, this._then);

  final SessionUpdate_FriendAccepted _self;
  final $Res Function(SessionUpdate_FriendAccepted) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? from = null,Object? nickname = null,}) {
  return _then(SessionUpdate_FriendAccepted(
from: null == from ? _self.from : from // ignore: cast_nullable_to_non_nullable
as String,nickname: null == nickname ? _self.nickname : nickname // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdate_ProfileUpdated extends SessionUpdate {
  const SessionUpdate_ProfileUpdated({required this.account, required this.nickname, required this.avatar}): super._();
  

 final  String account;
 final  String nickname;
 final  String avatar;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_ProfileUpdatedCopyWith<SessionUpdate_ProfileUpdated> get copyWith => _$SessionUpdate_ProfileUpdatedCopyWithImpl<SessionUpdate_ProfileUpdated>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_ProfileUpdated&&(identical(other.account, account) || other.account == account)&&(identical(other.nickname, nickname) || other.nickname == nickname)&&(identical(other.avatar, avatar) || other.avatar == avatar));
}


@override
int get hashCode => Object.hash(runtimeType,account,nickname,avatar);

@override
String toString() {
  return 'SessionUpdate.profileUpdated(account: $account, nickname: $nickname, avatar: $avatar)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_ProfileUpdatedCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_ProfileUpdatedCopyWith(SessionUpdate_ProfileUpdated value, $Res Function(SessionUpdate_ProfileUpdated) _then) = _$SessionUpdate_ProfileUpdatedCopyWithImpl;
@useResult
$Res call({
 String account, String nickname, String avatar
});




}
/// @nodoc
class _$SessionUpdate_ProfileUpdatedCopyWithImpl<$Res>
    implements $SessionUpdate_ProfileUpdatedCopyWith<$Res> {
  _$SessionUpdate_ProfileUpdatedCopyWithImpl(this._self, this._then);

  final SessionUpdate_ProfileUpdated _self;
  final $Res Function(SessionUpdate_ProfileUpdated) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? account = null,Object? nickname = null,Object? avatar = null,}) {
  return _then(SessionUpdate_ProfileUpdated(
account: null == account ? _self.account : account // ignore: cast_nullable_to_non_nullable
as String,nickname: null == nickname ? _self.nickname : nickname // ignore: cast_nullable_to_non_nullable
as String,avatar: null == avatar ? _self.avatar : avatar // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdate_Presence extends SessionUpdate {
  const SessionUpdate_Presence({required this.account, required this.status, required this.lastSeen}): super._();
  

 final  String account;
 final  int status;
 final  PlatformInt64 lastSeen;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_PresenceCopyWith<SessionUpdate_Presence> get copyWith => _$SessionUpdate_PresenceCopyWithImpl<SessionUpdate_Presence>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_Presence&&(identical(other.account, account) || other.account == account)&&(identical(other.status, status) || other.status == status)&&(identical(other.lastSeen, lastSeen) || other.lastSeen == lastSeen));
}


@override
int get hashCode => Object.hash(runtimeType,account,status,lastSeen);

@override
String toString() {
  return 'SessionUpdate.presence(account: $account, status: $status, lastSeen: $lastSeen)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_PresenceCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_PresenceCopyWith(SessionUpdate_Presence value, $Res Function(SessionUpdate_Presence) _then) = _$SessionUpdate_PresenceCopyWithImpl;
@useResult
$Res call({
 String account, int status, PlatformInt64 lastSeen
});




}
/// @nodoc
class _$SessionUpdate_PresenceCopyWithImpl<$Res>
    implements $SessionUpdate_PresenceCopyWith<$Res> {
  _$SessionUpdate_PresenceCopyWithImpl(this._self, this._then);

  final SessionUpdate_Presence _self;
  final $Res Function(SessionUpdate_Presence) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? account = null,Object? status = null,Object? lastSeen = null,}) {
  return _then(SessionUpdate_Presence(
account: null == account ? _self.account : account // ignore: cast_nullable_to_non_nullable
as String,status: null == status ? _self.status : status // ignore: cast_nullable_to_non_nullable
as int,lastSeen: null == lastSeen ? _self.lastSeen : lastSeen // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class SessionUpdate_Typing extends SessionUpdate {
  const SessionUpdate_Typing({required this.typer, required this.dest, required this.kind, required this.active}): super._();
  

 final  String typer;
 final  String dest;
 final  int kind;
 final  bool active;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_TypingCopyWith<SessionUpdate_Typing> get copyWith => _$SessionUpdate_TypingCopyWithImpl<SessionUpdate_Typing>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_Typing&&(identical(other.typer, typer) || other.typer == typer)&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.active, active) || other.active == active));
}


@override
int get hashCode => Object.hash(runtimeType,typer,dest,kind,active);

@override
String toString() {
  return 'SessionUpdate.typing(typer: $typer, dest: $dest, kind: $kind, active: $active)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_TypingCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_TypingCopyWith(SessionUpdate_Typing value, $Res Function(SessionUpdate_Typing) _then) = _$SessionUpdate_TypingCopyWithImpl;
@useResult
$Res call({
 String typer, String dest, int kind, bool active
});




}
/// @nodoc
class _$SessionUpdate_TypingCopyWithImpl<$Res>
    implements $SessionUpdate_TypingCopyWith<$Res> {
  _$SessionUpdate_TypingCopyWithImpl(this._self, this._then);

  final SessionUpdate_Typing _self;
  final $Res Function(SessionUpdate_Typing) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? typer = null,Object? dest = null,Object? kind = null,Object? active = null,}) {
  return _then(SessionUpdate_Typing(
typer: null == typer ? _self.typer : typer // ignore: cast_nullable_to_non_nullable
as String,dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,active: null == active ? _self.active : active // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class SessionUpdate_ReceiptRead extends SessionUpdate {
  const SessionUpdate_ReceiptRead({required this.reader, required this.dest, required this.kind, required this.messageId}): super._();
  

 final  String reader;
 final  String dest;
 final  int kind;
 final  PlatformInt64 messageId;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_ReceiptReadCopyWith<SessionUpdate_ReceiptRead> get copyWith => _$SessionUpdate_ReceiptReadCopyWithImpl<SessionUpdate_ReceiptRead>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_ReceiptRead&&(identical(other.reader, reader) || other.reader == reader)&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.messageId, messageId) || other.messageId == messageId));
}


@override
int get hashCode => Object.hash(runtimeType,reader,dest,kind,messageId);

@override
String toString() {
  return 'SessionUpdate.receiptRead(reader: $reader, dest: $dest, kind: $kind, messageId: $messageId)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_ReceiptReadCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_ReceiptReadCopyWith(SessionUpdate_ReceiptRead value, $Res Function(SessionUpdate_ReceiptRead) _then) = _$SessionUpdate_ReceiptReadCopyWithImpl;
@useResult
$Res call({
 String reader, String dest, int kind, PlatformInt64 messageId
});




}
/// @nodoc
class _$SessionUpdate_ReceiptReadCopyWithImpl<$Res>
    implements $SessionUpdate_ReceiptReadCopyWith<$Res> {
  _$SessionUpdate_ReceiptReadCopyWithImpl(this._self, this._then);

  final SessionUpdate_ReceiptRead _self;
  final $Res Function(SessionUpdate_ReceiptRead) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reader = null,Object? dest = null,Object? kind = null,Object? messageId = null,}) {
  return _then(SessionUpdate_ReceiptRead(
reader: null == reader ? _self.reader : reader // ignore: cast_nullable_to_non_nullable
as String,dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,messageId: null == messageId ? _self.messageId : messageId // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class SessionUpdate_GroupCreate extends SessionUpdate {
  const SessionUpdate_GroupCreate({required this.groupId, required  List<String> members}): _members = members,super._();
  

 final  String groupId;
 final  List<String> _members;
 List<String> get members {
  if (_members is EqualUnmodifiableListView) return _members;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_members);
}


/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_GroupCreateCopyWith<SessionUpdate_GroupCreate> get copyWith => _$SessionUpdate_GroupCreateCopyWithImpl<SessionUpdate_GroupCreate>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_GroupCreate&&(identical(other.groupId, groupId) || other.groupId == groupId)&&const DeepCollectionEquality().equals(other._members, _members));
}


@override
int get hashCode => Object.hash(runtimeType,groupId,const DeepCollectionEquality().hash(_members));

@override
String toString() {
  return 'SessionUpdate.groupCreate(groupId: $groupId, members: $members)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_GroupCreateCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_GroupCreateCopyWith(SessionUpdate_GroupCreate value, $Res Function(SessionUpdate_GroupCreate) _then) = _$SessionUpdate_GroupCreateCopyWithImpl;
@useResult
$Res call({
 String groupId, List<String> members
});




}
/// @nodoc
class _$SessionUpdate_GroupCreateCopyWithImpl<$Res>
    implements $SessionUpdate_GroupCreateCopyWith<$Res> {
  _$SessionUpdate_GroupCreateCopyWithImpl(this._self, this._then);

  final SessionUpdate_GroupCreate _self;
  final $Res Function(SessionUpdate_GroupCreate) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? groupId = null,Object? members = null,}) {
  return _then(SessionUpdate_GroupCreate(
groupId: null == groupId ? _self.groupId : groupId // ignore: cast_nullable_to_non_nullable
as String,members: null == members ? _self._members : members // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class SessionUpdate_ContactsChanged extends SessionUpdate {
  const SessionUpdate_ContactsChanged({required  List<Person> contacts}): _contacts = contacts,super._();
  

 final  List<Person> _contacts;
 List<Person> get contacts {
  if (_contacts is EqualUnmodifiableListView) return _contacts;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_contacts);
}


/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_ContactsChangedCopyWith<SessionUpdate_ContactsChanged> get copyWith => _$SessionUpdate_ContactsChangedCopyWithImpl<SessionUpdate_ContactsChanged>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_ContactsChanged&&const DeepCollectionEquality().equals(other._contacts, _contacts));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_contacts));

@override
String toString() {
  return 'SessionUpdate.contactsChanged(contacts: $contacts)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_ContactsChangedCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_ContactsChangedCopyWith(SessionUpdate_ContactsChanged value, $Res Function(SessionUpdate_ContactsChanged) _then) = _$SessionUpdate_ContactsChangedCopyWithImpl;
@useResult
$Res call({
 List<Person> contacts
});




}
/// @nodoc
class _$SessionUpdate_ContactsChangedCopyWithImpl<$Res>
    implements $SessionUpdate_ContactsChangedCopyWith<$Res> {
  _$SessionUpdate_ContactsChangedCopyWithImpl(this._self, this._then);

  final SessionUpdate_ContactsChanged _self;
  final $Res Function(SessionUpdate_ContactsChanged) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? contacts = null,}) {
  return _then(SessionUpdate_ContactsChanged(
contacts: null == contacts ? _self._contacts : contacts // ignore: cast_nullable_to_non_nullable
as List<Person>,
  ));
}


}

/// @nodoc


class SessionUpdate_AgentTurn extends SessionUpdate {
  const SessionUpdate_AgentTurn({required this.dest, required this.state, required this.text}): super._();
  

 final  String dest;
 final  AgentTurnState state;
 final  String text;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_AgentTurnCopyWith<SessionUpdate_AgentTurn> get copyWith => _$SessionUpdate_AgentTurnCopyWithImpl<SessionUpdate_AgentTurn>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_AgentTurn&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.state, state) || other.state == state)&&(identical(other.text, text) || other.text == text));
}


@override
int get hashCode => Object.hash(runtimeType,dest,state,text);

@override
String toString() {
  return 'SessionUpdate.agentTurn(dest: $dest, state: $state, text: $text)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_AgentTurnCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_AgentTurnCopyWith(SessionUpdate_AgentTurn value, $Res Function(SessionUpdate_AgentTurn) _then) = _$SessionUpdate_AgentTurnCopyWithImpl;
@useResult
$Res call({
 String dest, AgentTurnState state, String text
});




}
/// @nodoc
class _$SessionUpdate_AgentTurnCopyWithImpl<$Res>
    implements $SessionUpdate_AgentTurnCopyWith<$Res> {
  _$SessionUpdate_AgentTurnCopyWithImpl(this._self, this._then);

  final SessionUpdate_AgentTurn _self;
  final $Res Function(SessionUpdate_AgentTurn) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? state = null,Object? text = null,}) {
  return _then(SessionUpdate_AgentTurn(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as AgentTurnState,text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdate_AgentCard extends SessionUpdate {
  const SessionUpdate_AgentCard({required this.dest, required this.card}): super._();
  

 final  String dest;
 final  AgentCard card;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_AgentCardCopyWith<SessionUpdate_AgentCard> get copyWith => _$SessionUpdate_AgentCardCopyWithImpl<SessionUpdate_AgentCard>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_AgentCard&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.card, card) || other.card == card));
}


@override
int get hashCode => Object.hash(runtimeType,dest,card);

@override
String toString() {
  return 'SessionUpdate.agentCard(dest: $dest, card: $card)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_AgentCardCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_AgentCardCopyWith(SessionUpdate_AgentCard value, $Res Function(SessionUpdate_AgentCard) _then) = _$SessionUpdate_AgentCardCopyWithImpl;
@useResult
$Res call({
 String dest, AgentCard card
});




}
/// @nodoc
class _$SessionUpdate_AgentCardCopyWithImpl<$Res>
    implements $SessionUpdate_AgentCardCopyWith<$Res> {
  _$SessionUpdate_AgentCardCopyWithImpl(this._self, this._then);

  final SessionUpdate_AgentCard _self;
  final $Res Function(SessionUpdate_AgentCard) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? card = null,}) {
  return _then(SessionUpdate_AgentCard(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,card: null == card ? _self.card : card // ignore: cast_nullable_to_non_nullable
as AgentCard,
  ));
}


}

/// @nodoc


class SessionUpdate_RustPanic extends SessionUpdate {
  const SessionUpdate_RustPanic({required this.message}): super._();
  

 final  String message;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdate_RustPanicCopyWith<SessionUpdate_RustPanic> get copyWith => _$SessionUpdate_RustPanicCopyWithImpl<SessionUpdate_RustPanic>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdate_RustPanic&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'SessionUpdate.rustPanic(message: $message)';
}


}

/// @nodoc
abstract mixin class $SessionUpdate_RustPanicCopyWith<$Res> implements $SessionUpdateCopyWith<$Res> {
  factory $SessionUpdate_RustPanicCopyWith(SessionUpdate_RustPanic value, $Res Function(SessionUpdate_RustPanic) _then) = _$SessionUpdate_RustPanicCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$SessionUpdate_RustPanicCopyWithImpl<$Res>
    implements $SessionUpdate_RustPanicCopyWith<$Res> {
  _$SessionUpdate_RustPanicCopyWithImpl(this._self, this._then);

  final SessionUpdate_RustPanic _self;
  final $Res Function(SessionUpdate_RustPanic) _then;

/// Create a copy of SessionUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(SessionUpdate_RustPanic(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$TimelineUpdate {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TimelineUpdate);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'TimelineUpdate()';
}


}

/// @nodoc
class $TimelineUpdateCopyWith<$Res>  {
$TimelineUpdateCopyWith(TimelineUpdate _, $Res Function(TimelineUpdate) __);
}


/// Adds pattern-matching-related methods to [TimelineUpdate].
extension TimelineUpdatePatterns on TimelineUpdate {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( TimelineUpdate_Snapshot value)?  snapshot,TResult Function( TimelineUpdate_Delta value)?  delta,TResult Function( TimelineUpdate_Resync value)?  resync,required TResult orElse(),}){
final _that = this;
switch (_that) {
case TimelineUpdate_Snapshot() when snapshot != null:
return snapshot(_that);case TimelineUpdate_Delta() when delta != null:
return delta(_that);case TimelineUpdate_Resync() when resync != null:
return resync(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( TimelineUpdate_Snapshot value)  snapshot,required TResult Function( TimelineUpdate_Delta value)  delta,required TResult Function( TimelineUpdate_Resync value)  resync,}){
final _that = this;
switch (_that) {
case TimelineUpdate_Snapshot():
return snapshot(_that);case TimelineUpdate_Delta():
return delta(_that);case TimelineUpdate_Resync():
return resync(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( TimelineUpdate_Snapshot value)?  snapshot,TResult? Function( TimelineUpdate_Delta value)?  delta,TResult? Function( TimelineUpdate_Resync value)?  resync,}){
final _that = this;
switch (_that) {
case TimelineUpdate_Snapshot() when snapshot != null:
return snapshot(_that);case TimelineUpdate_Delta() when delta != null:
return delta(_that);case TimelineUpdate_Resync() when resync != null:
return resync(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( TimelineSnapshot snapshot)?  snapshot,TResult Function( TimelineDelta delta)?  delta,TResult Function( String dest,  String reason)?  resync,required TResult orElse(),}) {final _that = this;
switch (_that) {
case TimelineUpdate_Snapshot() when snapshot != null:
return snapshot(_that.snapshot);case TimelineUpdate_Delta() when delta != null:
return delta(_that.delta);case TimelineUpdate_Resync() when resync != null:
return resync(_that.dest,_that.reason);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( TimelineSnapshot snapshot)  snapshot,required TResult Function( TimelineDelta delta)  delta,required TResult Function( String dest,  String reason)  resync,}) {final _that = this;
switch (_that) {
case TimelineUpdate_Snapshot():
return snapshot(_that.snapshot);case TimelineUpdate_Delta():
return delta(_that.delta);case TimelineUpdate_Resync():
return resync(_that.dest,_that.reason);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( TimelineSnapshot snapshot)?  snapshot,TResult? Function( TimelineDelta delta)?  delta,TResult? Function( String dest,  String reason)?  resync,}) {final _that = this;
switch (_that) {
case TimelineUpdate_Snapshot() when snapshot != null:
return snapshot(_that.snapshot);case TimelineUpdate_Delta() when delta != null:
return delta(_that.delta);case TimelineUpdate_Resync() when resync != null:
return resync(_that.dest,_that.reason);case _:
  return null;

}
}

}

/// @nodoc


class TimelineUpdate_Snapshot extends TimelineUpdate {
  const TimelineUpdate_Snapshot({required this.snapshot}): super._();
  

 final  TimelineSnapshot snapshot;

/// Create a copy of TimelineUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TimelineUpdate_SnapshotCopyWith<TimelineUpdate_Snapshot> get copyWith => _$TimelineUpdate_SnapshotCopyWithImpl<TimelineUpdate_Snapshot>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TimelineUpdate_Snapshot&&(identical(other.snapshot, snapshot) || other.snapshot == snapshot));
}


@override
int get hashCode => Object.hash(runtimeType,snapshot);

@override
String toString() {
  return 'TimelineUpdate.snapshot(snapshot: $snapshot)';
}


}

/// @nodoc
abstract mixin class $TimelineUpdate_SnapshotCopyWith<$Res> implements $TimelineUpdateCopyWith<$Res> {
  factory $TimelineUpdate_SnapshotCopyWith(TimelineUpdate_Snapshot value, $Res Function(TimelineUpdate_Snapshot) _then) = _$TimelineUpdate_SnapshotCopyWithImpl;
@useResult
$Res call({
 TimelineSnapshot snapshot
});




}
/// @nodoc
class _$TimelineUpdate_SnapshotCopyWithImpl<$Res>
    implements $TimelineUpdate_SnapshotCopyWith<$Res> {
  _$TimelineUpdate_SnapshotCopyWithImpl(this._self, this._then);

  final TimelineUpdate_Snapshot _self;
  final $Res Function(TimelineUpdate_Snapshot) _then;

/// Create a copy of TimelineUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? snapshot = null,}) {
  return _then(TimelineUpdate_Snapshot(
snapshot: null == snapshot ? _self.snapshot : snapshot // ignore: cast_nullable_to_non_nullable
as TimelineSnapshot,
  ));
}


}

/// @nodoc


class TimelineUpdate_Delta extends TimelineUpdate {
  const TimelineUpdate_Delta({required this.delta}): super._();
  

 final  TimelineDelta delta;

/// Create a copy of TimelineUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TimelineUpdate_DeltaCopyWith<TimelineUpdate_Delta> get copyWith => _$TimelineUpdate_DeltaCopyWithImpl<TimelineUpdate_Delta>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TimelineUpdate_Delta&&(identical(other.delta, delta) || other.delta == delta));
}


@override
int get hashCode => Object.hash(runtimeType,delta);

@override
String toString() {
  return 'TimelineUpdate.delta(delta: $delta)';
}


}

/// @nodoc
abstract mixin class $TimelineUpdate_DeltaCopyWith<$Res> implements $TimelineUpdateCopyWith<$Res> {
  factory $TimelineUpdate_DeltaCopyWith(TimelineUpdate_Delta value, $Res Function(TimelineUpdate_Delta) _then) = _$TimelineUpdate_DeltaCopyWithImpl;
@useResult
$Res call({
 TimelineDelta delta
});




}
/// @nodoc
class _$TimelineUpdate_DeltaCopyWithImpl<$Res>
    implements $TimelineUpdate_DeltaCopyWith<$Res> {
  _$TimelineUpdate_DeltaCopyWithImpl(this._self, this._then);

  final TimelineUpdate_Delta _self;
  final $Res Function(TimelineUpdate_Delta) _then;

/// Create a copy of TimelineUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? delta = null,}) {
  return _then(TimelineUpdate_Delta(
delta: null == delta ? _self.delta : delta // ignore: cast_nullable_to_non_nullable
as TimelineDelta,
  ));
}


}

/// @nodoc


class TimelineUpdate_Resync extends TimelineUpdate {
  const TimelineUpdate_Resync({required this.dest, required this.reason}): super._();
  

 final  String dest;
 final  String reason;

/// Create a copy of TimelineUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TimelineUpdate_ResyncCopyWith<TimelineUpdate_Resync> get copyWith => _$TimelineUpdate_ResyncCopyWithImpl<TimelineUpdate_Resync>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TimelineUpdate_Resync&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,dest,reason);

@override
String toString() {
  return 'TimelineUpdate.resync(dest: $dest, reason: $reason)';
}


}

/// @nodoc
abstract mixin class $TimelineUpdate_ResyncCopyWith<$Res> implements $TimelineUpdateCopyWith<$Res> {
  factory $TimelineUpdate_ResyncCopyWith(TimelineUpdate_Resync value, $Res Function(TimelineUpdate_Resync) _then) = _$TimelineUpdate_ResyncCopyWithImpl;
@useResult
$Res call({
 String dest, String reason
});




}
/// @nodoc
class _$TimelineUpdate_ResyncCopyWithImpl<$Res>
    implements $TimelineUpdate_ResyncCopyWith<$Res> {
  _$TimelineUpdate_ResyncCopyWithImpl(this._self, this._then);

  final TimelineUpdate_Resync _self;
  final $Res Function(TimelineUpdate_Resync) _then;

/// Create a copy of TimelineUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? reason = null,}) {
  return _then(TimelineUpdate_Resync(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$TokenPersist {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TokenPersist);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'TokenPersist()';
}


}

/// @nodoc
class $TokenPersistCopyWith<$Res>  {
$TokenPersistCopyWith(TokenPersist _, $Res Function(TokenPersist) __);
}


/// Adds pattern-matching-related methods to [TokenPersist].
extension TokenPersistPatterns on TokenPersist {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( TokenPersist_Write value)?  write,TResult Function( TokenPersist_Clear value)?  clear,required TResult orElse(),}){
final _that = this;
switch (_that) {
case TokenPersist_Write() when write != null:
return write(_that);case TokenPersist_Clear() when clear != null:
return clear(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( TokenPersist_Write value)  write,required TResult Function( TokenPersist_Clear value)  clear,}){
final _that = this;
switch (_that) {
case TokenPersist_Write():
return write(_that);case TokenPersist_Clear():
return clear(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( TokenPersist_Write value)?  write,TResult? Function( TokenPersist_Clear value)?  clear,}){
final _that = this;
switch (_that) {
case TokenPersist_Write() when write != null:
return write(_that);case TokenPersist_Clear() when clear != null:
return clear(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String token)?  write,TResult Function()?  clear,required TResult orElse(),}) {final _that = this;
switch (_that) {
case TokenPersist_Write() when write != null:
return write(_that.token);case TokenPersist_Clear() when clear != null:
return clear();case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String token)  write,required TResult Function()  clear,}) {final _that = this;
switch (_that) {
case TokenPersist_Write():
return write(_that.token);case TokenPersist_Clear():
return clear();}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String token)?  write,TResult? Function()?  clear,}) {final _that = this;
switch (_that) {
case TokenPersist_Write() when write != null:
return write(_that.token);case TokenPersist_Clear() when clear != null:
return clear();case _:
  return null;

}
}

}

/// @nodoc


class TokenPersist_Write extends TokenPersist {
  const TokenPersist_Write({required this.token}): super._();
  

 final  String token;

/// Create a copy of TokenPersist
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TokenPersist_WriteCopyWith<TokenPersist_Write> get copyWith => _$TokenPersist_WriteCopyWithImpl<TokenPersist_Write>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TokenPersist_Write&&(identical(other.token, token) || other.token == token));
}


@override
int get hashCode => Object.hash(runtimeType,token);

@override
String toString() {
  return 'TokenPersist.write(token: $token)';
}


}

/// @nodoc
abstract mixin class $TokenPersist_WriteCopyWith<$Res> implements $TokenPersistCopyWith<$Res> {
  factory $TokenPersist_WriteCopyWith(TokenPersist_Write value, $Res Function(TokenPersist_Write) _then) = _$TokenPersist_WriteCopyWithImpl;
@useResult
$Res call({
 String token
});




}
/// @nodoc
class _$TokenPersist_WriteCopyWithImpl<$Res>
    implements $TokenPersist_WriteCopyWith<$Res> {
  _$TokenPersist_WriteCopyWithImpl(this._self, this._then);

  final TokenPersist_Write _self;
  final $Res Function(TokenPersist_Write) _then;

/// Create a copy of TokenPersist
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? token = null,}) {
  return _then(TokenPersist_Write(
token: null == token ? _self.token : token // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class TokenPersist_Clear extends TokenPersist {
  const TokenPersist_Clear(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TokenPersist_Clear);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'TokenPersist.clear()';
}


}




/// @nodoc
mixin _$UiCommand {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'UiCommand()';
}


}

/// @nodoc
class $UiCommandCopyWith<$Res>  {
$UiCommandCopyWith(UiCommand _, $Res Function(UiCommand) __);
}


/// Adds pattern-matching-related methods to [UiCommand].
extension UiCommandPatterns on UiCommand {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( UiCommand_SendText value)?  sendText,TResult Function( UiCommand_SendMedia value)?  sendMedia,TResult Function( UiCommand_RetrySend value)?  retrySend,TResult Function( UiCommand_CancelSend value)?  cancelSend,TResult Function( UiCommand_MarkThreadRead value)?  markThreadRead,TResult Function( UiCommand_DeleteThread value)?  deleteThread,TResult Function( UiCommand_FriendRequest value)?  friendRequest,TResult Function( UiCommand_FriendAccept value)?  friendAccept,TResult Function( UiCommand_FriendReject value)?  friendReject,TResult Function( UiCommand_FriendRemove value)?  friendRemove,TResult Function( UiCommand_AgentEnqueueTurn value)?  agentEnqueueTurn,TResult Function( UiCommand_AgentRespondPermission value)?  agentRespondPermission,TResult Function( UiCommand_AgentAbortTurn value)?  agentAbortTurn,TResult Function( UiCommand_AgentRunResult value)?  agentRunResult,TResult Function( UiCommand_SettingsPatch value)?  settingsPatch,required TResult orElse(),}){
final _that = this;
switch (_that) {
case UiCommand_SendText() when sendText != null:
return sendText(_that);case UiCommand_SendMedia() when sendMedia != null:
return sendMedia(_that);case UiCommand_RetrySend() when retrySend != null:
return retrySend(_that);case UiCommand_CancelSend() when cancelSend != null:
return cancelSend(_that);case UiCommand_MarkThreadRead() when markThreadRead != null:
return markThreadRead(_that);case UiCommand_DeleteThread() when deleteThread != null:
return deleteThread(_that);case UiCommand_FriendRequest() when friendRequest != null:
return friendRequest(_that);case UiCommand_FriendAccept() when friendAccept != null:
return friendAccept(_that);case UiCommand_FriendReject() when friendReject != null:
return friendReject(_that);case UiCommand_FriendRemove() when friendRemove != null:
return friendRemove(_that);case UiCommand_AgentEnqueueTurn() when agentEnqueueTurn != null:
return agentEnqueueTurn(_that);case UiCommand_AgentRespondPermission() when agentRespondPermission != null:
return agentRespondPermission(_that);case UiCommand_AgentAbortTurn() when agentAbortTurn != null:
return agentAbortTurn(_that);case UiCommand_AgentRunResult() when agentRunResult != null:
return agentRunResult(_that);case UiCommand_SettingsPatch() when settingsPatch != null:
return settingsPatch(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( UiCommand_SendText value)  sendText,required TResult Function( UiCommand_SendMedia value)  sendMedia,required TResult Function( UiCommand_RetrySend value)  retrySend,required TResult Function( UiCommand_CancelSend value)  cancelSend,required TResult Function( UiCommand_MarkThreadRead value)  markThreadRead,required TResult Function( UiCommand_DeleteThread value)  deleteThread,required TResult Function( UiCommand_FriendRequest value)  friendRequest,required TResult Function( UiCommand_FriendAccept value)  friendAccept,required TResult Function( UiCommand_FriendReject value)  friendReject,required TResult Function( UiCommand_FriendRemove value)  friendRemove,required TResult Function( UiCommand_AgentEnqueueTurn value)  agentEnqueueTurn,required TResult Function( UiCommand_AgentRespondPermission value)  agentRespondPermission,required TResult Function( UiCommand_AgentAbortTurn value)  agentAbortTurn,required TResult Function( UiCommand_AgentRunResult value)  agentRunResult,required TResult Function( UiCommand_SettingsPatch value)  settingsPatch,}){
final _that = this;
switch (_that) {
case UiCommand_SendText():
return sendText(_that);case UiCommand_SendMedia():
return sendMedia(_that);case UiCommand_RetrySend():
return retrySend(_that);case UiCommand_CancelSend():
return cancelSend(_that);case UiCommand_MarkThreadRead():
return markThreadRead(_that);case UiCommand_DeleteThread():
return deleteThread(_that);case UiCommand_FriendRequest():
return friendRequest(_that);case UiCommand_FriendAccept():
return friendAccept(_that);case UiCommand_FriendReject():
return friendReject(_that);case UiCommand_FriendRemove():
return friendRemove(_that);case UiCommand_AgentEnqueueTurn():
return agentEnqueueTurn(_that);case UiCommand_AgentRespondPermission():
return agentRespondPermission(_that);case UiCommand_AgentAbortTurn():
return agentAbortTurn(_that);case UiCommand_AgentRunResult():
return agentRunResult(_that);case UiCommand_SettingsPatch():
return settingsPatch(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( UiCommand_SendText value)?  sendText,TResult? Function( UiCommand_SendMedia value)?  sendMedia,TResult? Function( UiCommand_RetrySend value)?  retrySend,TResult? Function( UiCommand_CancelSend value)?  cancelSend,TResult? Function( UiCommand_MarkThreadRead value)?  markThreadRead,TResult? Function( UiCommand_DeleteThread value)?  deleteThread,TResult? Function( UiCommand_FriendRequest value)?  friendRequest,TResult? Function( UiCommand_FriendAccept value)?  friendAccept,TResult? Function( UiCommand_FriendReject value)?  friendReject,TResult? Function( UiCommand_FriendRemove value)?  friendRemove,TResult? Function( UiCommand_AgentEnqueueTurn value)?  agentEnqueueTurn,TResult? Function( UiCommand_AgentRespondPermission value)?  agentRespondPermission,TResult? Function( UiCommand_AgentAbortTurn value)?  agentAbortTurn,TResult? Function( UiCommand_AgentRunResult value)?  agentRunResult,TResult? Function( UiCommand_SettingsPatch value)?  settingsPatch,}){
final _that = this;
switch (_that) {
case UiCommand_SendText() when sendText != null:
return sendText(_that);case UiCommand_SendMedia() when sendMedia != null:
return sendMedia(_that);case UiCommand_RetrySend() when retrySend != null:
return retrySend(_that);case UiCommand_CancelSend() when cancelSend != null:
return cancelSend(_that);case UiCommand_MarkThreadRead() when markThreadRead != null:
return markThreadRead(_that);case UiCommand_DeleteThread() when deleteThread != null:
return deleteThread(_that);case UiCommand_FriendRequest() when friendRequest != null:
return friendRequest(_that);case UiCommand_FriendAccept() when friendAccept != null:
return friendAccept(_that);case UiCommand_FriendReject() when friendReject != null:
return friendReject(_that);case UiCommand_FriendRemove() when friendRemove != null:
return friendRemove(_that);case UiCommand_AgentEnqueueTurn() when agentEnqueueTurn != null:
return agentEnqueueTurn(_that);case UiCommand_AgentRespondPermission() when agentRespondPermission != null:
return agentRespondPermission(_that);case UiCommand_AgentAbortTurn() when agentAbortTurn != null:
return agentAbortTurn(_that);case UiCommand_AgentRunResult() when agentRunResult != null:
return agentRunResult(_that);case UiCommand_SettingsPatch() when settingsPatch != null:
return settingsPatch(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String dest,  String text,  int kind)?  sendText,TResult Function( String dest,  String path,  String mime,  int width,  int height,  PlatformInt64 byteSize,  int kind)?  sendMedia,TResult Function( String clientId)?  retrySend,TResult Function( String clientId)?  cancelSend,TResult Function( String dest,  int kind,  PlatformInt64 visibleMessageId)?  markThreadRead,TResult Function( String dest)?  deleteThread,TResult Function( String dest)?  friendRequest,TResult Function( String dest)?  friendAccept,TResult Function( String dest)?  friendReject,TResult Function( String dest)?  friendRemove,TResult Function( String dest,  String text,  PlatformInt64 inReplyTo)?  agentEnqueueTurn,TResult Function( String dest,  String callId,  String permission)?  agentRespondPermission,TResult Function( String dest)?  agentAbortTurn,TResult Function( String dest,  String profileId,  BigInt epoch,  String output,  String? error)?  agentRunResult,TResult Function( String? wsUrl,  String? httpOrigin,  String? env)?  settingsPatch,required TResult orElse(),}) {final _that = this;
switch (_that) {
case UiCommand_SendText() when sendText != null:
return sendText(_that.dest,_that.text,_that.kind);case UiCommand_SendMedia() when sendMedia != null:
return sendMedia(_that.dest,_that.path,_that.mime,_that.width,_that.height,_that.byteSize,_that.kind);case UiCommand_RetrySend() when retrySend != null:
return retrySend(_that.clientId);case UiCommand_CancelSend() when cancelSend != null:
return cancelSend(_that.clientId);case UiCommand_MarkThreadRead() when markThreadRead != null:
return markThreadRead(_that.dest,_that.kind,_that.visibleMessageId);case UiCommand_DeleteThread() when deleteThread != null:
return deleteThread(_that.dest);case UiCommand_FriendRequest() when friendRequest != null:
return friendRequest(_that.dest);case UiCommand_FriendAccept() when friendAccept != null:
return friendAccept(_that.dest);case UiCommand_FriendReject() when friendReject != null:
return friendReject(_that.dest);case UiCommand_FriendRemove() when friendRemove != null:
return friendRemove(_that.dest);case UiCommand_AgentEnqueueTurn() when agentEnqueueTurn != null:
return agentEnqueueTurn(_that.dest,_that.text,_that.inReplyTo);case UiCommand_AgentRespondPermission() when agentRespondPermission != null:
return agentRespondPermission(_that.dest,_that.callId,_that.permission);case UiCommand_AgentAbortTurn() when agentAbortTurn != null:
return agentAbortTurn(_that.dest);case UiCommand_AgentRunResult() when agentRunResult != null:
return agentRunResult(_that.dest,_that.profileId,_that.epoch,_that.output,_that.error);case UiCommand_SettingsPatch() when settingsPatch != null:
return settingsPatch(_that.wsUrl,_that.httpOrigin,_that.env);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String dest,  String text,  int kind)  sendText,required TResult Function( String dest,  String path,  String mime,  int width,  int height,  PlatformInt64 byteSize,  int kind)  sendMedia,required TResult Function( String clientId)  retrySend,required TResult Function( String clientId)  cancelSend,required TResult Function( String dest,  int kind,  PlatformInt64 visibleMessageId)  markThreadRead,required TResult Function( String dest)  deleteThread,required TResult Function( String dest)  friendRequest,required TResult Function( String dest)  friendAccept,required TResult Function( String dest)  friendReject,required TResult Function( String dest)  friendRemove,required TResult Function( String dest,  String text,  PlatformInt64 inReplyTo)  agentEnqueueTurn,required TResult Function( String dest,  String callId,  String permission)  agentRespondPermission,required TResult Function( String dest)  agentAbortTurn,required TResult Function( String dest,  String profileId,  BigInt epoch,  String output,  String? error)  agentRunResult,required TResult Function( String? wsUrl,  String? httpOrigin,  String? env)  settingsPatch,}) {final _that = this;
switch (_that) {
case UiCommand_SendText():
return sendText(_that.dest,_that.text,_that.kind);case UiCommand_SendMedia():
return sendMedia(_that.dest,_that.path,_that.mime,_that.width,_that.height,_that.byteSize,_that.kind);case UiCommand_RetrySend():
return retrySend(_that.clientId);case UiCommand_CancelSend():
return cancelSend(_that.clientId);case UiCommand_MarkThreadRead():
return markThreadRead(_that.dest,_that.kind,_that.visibleMessageId);case UiCommand_DeleteThread():
return deleteThread(_that.dest);case UiCommand_FriendRequest():
return friendRequest(_that.dest);case UiCommand_FriendAccept():
return friendAccept(_that.dest);case UiCommand_FriendReject():
return friendReject(_that.dest);case UiCommand_FriendRemove():
return friendRemove(_that.dest);case UiCommand_AgentEnqueueTurn():
return agentEnqueueTurn(_that.dest,_that.text,_that.inReplyTo);case UiCommand_AgentRespondPermission():
return agentRespondPermission(_that.dest,_that.callId,_that.permission);case UiCommand_AgentAbortTurn():
return agentAbortTurn(_that.dest);case UiCommand_AgentRunResult():
return agentRunResult(_that.dest,_that.profileId,_that.epoch,_that.output,_that.error);case UiCommand_SettingsPatch():
return settingsPatch(_that.wsUrl,_that.httpOrigin,_that.env);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String dest,  String text,  int kind)?  sendText,TResult? Function( String dest,  String path,  String mime,  int width,  int height,  PlatformInt64 byteSize,  int kind)?  sendMedia,TResult? Function( String clientId)?  retrySend,TResult? Function( String clientId)?  cancelSend,TResult? Function( String dest,  int kind,  PlatformInt64 visibleMessageId)?  markThreadRead,TResult? Function( String dest)?  deleteThread,TResult? Function( String dest)?  friendRequest,TResult? Function( String dest)?  friendAccept,TResult? Function( String dest)?  friendReject,TResult? Function( String dest)?  friendRemove,TResult? Function( String dest,  String text,  PlatformInt64 inReplyTo)?  agentEnqueueTurn,TResult? Function( String dest,  String callId,  String permission)?  agentRespondPermission,TResult? Function( String dest)?  agentAbortTurn,TResult? Function( String dest,  String profileId,  BigInt epoch,  String output,  String? error)?  agentRunResult,TResult? Function( String? wsUrl,  String? httpOrigin,  String? env)?  settingsPatch,}) {final _that = this;
switch (_that) {
case UiCommand_SendText() when sendText != null:
return sendText(_that.dest,_that.text,_that.kind);case UiCommand_SendMedia() when sendMedia != null:
return sendMedia(_that.dest,_that.path,_that.mime,_that.width,_that.height,_that.byteSize,_that.kind);case UiCommand_RetrySend() when retrySend != null:
return retrySend(_that.clientId);case UiCommand_CancelSend() when cancelSend != null:
return cancelSend(_that.clientId);case UiCommand_MarkThreadRead() when markThreadRead != null:
return markThreadRead(_that.dest,_that.kind,_that.visibleMessageId);case UiCommand_DeleteThread() when deleteThread != null:
return deleteThread(_that.dest);case UiCommand_FriendRequest() when friendRequest != null:
return friendRequest(_that.dest);case UiCommand_FriendAccept() when friendAccept != null:
return friendAccept(_that.dest);case UiCommand_FriendReject() when friendReject != null:
return friendReject(_that.dest);case UiCommand_FriendRemove() when friendRemove != null:
return friendRemove(_that.dest);case UiCommand_AgentEnqueueTurn() when agentEnqueueTurn != null:
return agentEnqueueTurn(_that.dest,_that.text,_that.inReplyTo);case UiCommand_AgentRespondPermission() when agentRespondPermission != null:
return agentRespondPermission(_that.dest,_that.callId,_that.permission);case UiCommand_AgentAbortTurn() when agentAbortTurn != null:
return agentAbortTurn(_that.dest);case UiCommand_AgentRunResult() when agentRunResult != null:
return agentRunResult(_that.dest,_that.profileId,_that.epoch,_that.output,_that.error);case UiCommand_SettingsPatch() when settingsPatch != null:
return settingsPatch(_that.wsUrl,_that.httpOrigin,_that.env);case _:
  return null;

}
}

}

/// @nodoc


class UiCommand_SendText extends UiCommand {
  const UiCommand_SendText({required this.dest, required this.text, required this.kind}): super._();
  

 final  String dest;
 final  String text;
 final  int kind;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_SendTextCopyWith<UiCommand_SendText> get copyWith => _$UiCommand_SendTextCopyWithImpl<UiCommand_SendText>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_SendText&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.text, text) || other.text == text)&&(identical(other.kind, kind) || other.kind == kind));
}


@override
int get hashCode => Object.hash(runtimeType,dest,text,kind);

@override
String toString() {
  return 'UiCommand.sendText(dest: $dest, text: $text, kind: $kind)';
}


}

/// @nodoc
abstract mixin class $UiCommand_SendTextCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_SendTextCopyWith(UiCommand_SendText value, $Res Function(UiCommand_SendText) _then) = _$UiCommand_SendTextCopyWithImpl;
@useResult
$Res call({
 String dest, String text, int kind
});




}
/// @nodoc
class _$UiCommand_SendTextCopyWithImpl<$Res>
    implements $UiCommand_SendTextCopyWith<$Res> {
  _$UiCommand_SendTextCopyWithImpl(this._self, this._then);

  final UiCommand_SendText _self;
  final $Res Function(UiCommand_SendText) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? text = null,Object? kind = null,}) {
  return _then(UiCommand_SendText(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class UiCommand_SendMedia extends UiCommand {
  const UiCommand_SendMedia({required this.dest, required this.path, required this.mime, required this.width, required this.height, required this.byteSize, required this.kind}): super._();
  

 final  String dest;
 final  String path;
 final  String mime;
 final  int width;
 final  int height;
 final  PlatformInt64 byteSize;
 final  int kind;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_SendMediaCopyWith<UiCommand_SendMedia> get copyWith => _$UiCommand_SendMediaCopyWithImpl<UiCommand_SendMedia>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_SendMedia&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.path, path) || other.path == path)&&(identical(other.mime, mime) || other.mime == mime)&&(identical(other.width, width) || other.width == width)&&(identical(other.height, height) || other.height == height)&&(identical(other.byteSize, byteSize) || other.byteSize == byteSize)&&(identical(other.kind, kind) || other.kind == kind));
}


@override
int get hashCode => Object.hash(runtimeType,dest,path,mime,width,height,byteSize,kind);

@override
String toString() {
  return 'UiCommand.sendMedia(dest: $dest, path: $path, mime: $mime, width: $width, height: $height, byteSize: $byteSize, kind: $kind)';
}


}

/// @nodoc
abstract mixin class $UiCommand_SendMediaCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_SendMediaCopyWith(UiCommand_SendMedia value, $Res Function(UiCommand_SendMedia) _then) = _$UiCommand_SendMediaCopyWithImpl;
@useResult
$Res call({
 String dest, String path, String mime, int width, int height, PlatformInt64 byteSize, int kind
});




}
/// @nodoc
class _$UiCommand_SendMediaCopyWithImpl<$Res>
    implements $UiCommand_SendMediaCopyWith<$Res> {
  _$UiCommand_SendMediaCopyWithImpl(this._self, this._then);

  final UiCommand_SendMedia _self;
  final $Res Function(UiCommand_SendMedia) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? path = null,Object? mime = null,Object? width = null,Object? height = null,Object? byteSize = null,Object? kind = null,}) {
  return _then(UiCommand_SendMedia(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,mime: null == mime ? _self.mime : mime // ignore: cast_nullable_to_non_nullable
as String,width: null == width ? _self.width : width // ignore: cast_nullable_to_non_nullable
as int,height: null == height ? _self.height : height // ignore: cast_nullable_to_non_nullable
as int,byteSize: null == byteSize ? _self.byteSize : byteSize // ignore: cast_nullable_to_non_nullable
as PlatformInt64,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class UiCommand_RetrySend extends UiCommand {
  const UiCommand_RetrySend({required this.clientId}): super._();
  

 final  String clientId;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_RetrySendCopyWith<UiCommand_RetrySend> get copyWith => _$UiCommand_RetrySendCopyWithImpl<UiCommand_RetrySend>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_RetrySend&&(identical(other.clientId, clientId) || other.clientId == clientId));
}


@override
int get hashCode => Object.hash(runtimeType,clientId);

@override
String toString() {
  return 'UiCommand.retrySend(clientId: $clientId)';
}


}

/// @nodoc
abstract mixin class $UiCommand_RetrySendCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_RetrySendCopyWith(UiCommand_RetrySend value, $Res Function(UiCommand_RetrySend) _then) = _$UiCommand_RetrySendCopyWithImpl;
@useResult
$Res call({
 String clientId
});




}
/// @nodoc
class _$UiCommand_RetrySendCopyWithImpl<$Res>
    implements $UiCommand_RetrySendCopyWith<$Res> {
  _$UiCommand_RetrySendCopyWithImpl(this._self, this._then);

  final UiCommand_RetrySend _self;
  final $Res Function(UiCommand_RetrySend) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? clientId = null,}) {
  return _then(UiCommand_RetrySend(
clientId: null == clientId ? _self.clientId : clientId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_CancelSend extends UiCommand {
  const UiCommand_CancelSend({required this.clientId}): super._();
  

 final  String clientId;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_CancelSendCopyWith<UiCommand_CancelSend> get copyWith => _$UiCommand_CancelSendCopyWithImpl<UiCommand_CancelSend>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_CancelSend&&(identical(other.clientId, clientId) || other.clientId == clientId));
}


@override
int get hashCode => Object.hash(runtimeType,clientId);

@override
String toString() {
  return 'UiCommand.cancelSend(clientId: $clientId)';
}


}

/// @nodoc
abstract mixin class $UiCommand_CancelSendCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_CancelSendCopyWith(UiCommand_CancelSend value, $Res Function(UiCommand_CancelSend) _then) = _$UiCommand_CancelSendCopyWithImpl;
@useResult
$Res call({
 String clientId
});




}
/// @nodoc
class _$UiCommand_CancelSendCopyWithImpl<$Res>
    implements $UiCommand_CancelSendCopyWith<$Res> {
  _$UiCommand_CancelSendCopyWithImpl(this._self, this._then);

  final UiCommand_CancelSend _self;
  final $Res Function(UiCommand_CancelSend) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? clientId = null,}) {
  return _then(UiCommand_CancelSend(
clientId: null == clientId ? _self.clientId : clientId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_MarkThreadRead extends UiCommand {
  const UiCommand_MarkThreadRead({required this.dest, required this.kind, required this.visibleMessageId}): super._();
  

 final  String dest;
 final  int kind;
 final  PlatformInt64 visibleMessageId;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_MarkThreadReadCopyWith<UiCommand_MarkThreadRead> get copyWith => _$UiCommand_MarkThreadReadCopyWithImpl<UiCommand_MarkThreadRead>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_MarkThreadRead&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.visibleMessageId, visibleMessageId) || other.visibleMessageId == visibleMessageId));
}


@override
int get hashCode => Object.hash(runtimeType,dest,kind,visibleMessageId);

@override
String toString() {
  return 'UiCommand.markThreadRead(dest: $dest, kind: $kind, visibleMessageId: $visibleMessageId)';
}


}

/// @nodoc
abstract mixin class $UiCommand_MarkThreadReadCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_MarkThreadReadCopyWith(UiCommand_MarkThreadRead value, $Res Function(UiCommand_MarkThreadRead) _then) = _$UiCommand_MarkThreadReadCopyWithImpl;
@useResult
$Res call({
 String dest, int kind, PlatformInt64 visibleMessageId
});




}
/// @nodoc
class _$UiCommand_MarkThreadReadCopyWithImpl<$Res>
    implements $UiCommand_MarkThreadReadCopyWith<$Res> {
  _$UiCommand_MarkThreadReadCopyWithImpl(this._self, this._then);

  final UiCommand_MarkThreadRead _self;
  final $Res Function(UiCommand_MarkThreadRead) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? kind = null,Object? visibleMessageId = null,}) {
  return _then(UiCommand_MarkThreadRead(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,visibleMessageId: null == visibleMessageId ? _self.visibleMessageId : visibleMessageId // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class UiCommand_DeleteThread extends UiCommand {
  const UiCommand_DeleteThread({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_DeleteThreadCopyWith<UiCommand_DeleteThread> get copyWith => _$UiCommand_DeleteThreadCopyWithImpl<UiCommand_DeleteThread>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_DeleteThread&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommand.deleteThread(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommand_DeleteThreadCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_DeleteThreadCopyWith(UiCommand_DeleteThread value, $Res Function(UiCommand_DeleteThread) _then) = _$UiCommand_DeleteThreadCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommand_DeleteThreadCopyWithImpl<$Res>
    implements $UiCommand_DeleteThreadCopyWith<$Res> {
  _$UiCommand_DeleteThreadCopyWithImpl(this._self, this._then);

  final UiCommand_DeleteThread _self;
  final $Res Function(UiCommand_DeleteThread) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommand_DeleteThread(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_FriendRequest extends UiCommand {
  const UiCommand_FriendRequest({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_FriendRequestCopyWith<UiCommand_FriendRequest> get copyWith => _$UiCommand_FriendRequestCopyWithImpl<UiCommand_FriendRequest>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_FriendRequest&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommand.friendRequest(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommand_FriendRequestCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_FriendRequestCopyWith(UiCommand_FriendRequest value, $Res Function(UiCommand_FriendRequest) _then) = _$UiCommand_FriendRequestCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommand_FriendRequestCopyWithImpl<$Res>
    implements $UiCommand_FriendRequestCopyWith<$Res> {
  _$UiCommand_FriendRequestCopyWithImpl(this._self, this._then);

  final UiCommand_FriendRequest _self;
  final $Res Function(UiCommand_FriendRequest) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommand_FriendRequest(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_FriendAccept extends UiCommand {
  const UiCommand_FriendAccept({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_FriendAcceptCopyWith<UiCommand_FriendAccept> get copyWith => _$UiCommand_FriendAcceptCopyWithImpl<UiCommand_FriendAccept>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_FriendAccept&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommand.friendAccept(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommand_FriendAcceptCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_FriendAcceptCopyWith(UiCommand_FriendAccept value, $Res Function(UiCommand_FriendAccept) _then) = _$UiCommand_FriendAcceptCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommand_FriendAcceptCopyWithImpl<$Res>
    implements $UiCommand_FriendAcceptCopyWith<$Res> {
  _$UiCommand_FriendAcceptCopyWithImpl(this._self, this._then);

  final UiCommand_FriendAccept _self;
  final $Res Function(UiCommand_FriendAccept) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommand_FriendAccept(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_FriendReject extends UiCommand {
  const UiCommand_FriendReject({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_FriendRejectCopyWith<UiCommand_FriendReject> get copyWith => _$UiCommand_FriendRejectCopyWithImpl<UiCommand_FriendReject>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_FriendReject&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommand.friendReject(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommand_FriendRejectCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_FriendRejectCopyWith(UiCommand_FriendReject value, $Res Function(UiCommand_FriendReject) _then) = _$UiCommand_FriendRejectCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommand_FriendRejectCopyWithImpl<$Res>
    implements $UiCommand_FriendRejectCopyWith<$Res> {
  _$UiCommand_FriendRejectCopyWithImpl(this._self, this._then);

  final UiCommand_FriendReject _self;
  final $Res Function(UiCommand_FriendReject) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommand_FriendReject(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_FriendRemove extends UiCommand {
  const UiCommand_FriendRemove({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_FriendRemoveCopyWith<UiCommand_FriendRemove> get copyWith => _$UiCommand_FriendRemoveCopyWithImpl<UiCommand_FriendRemove>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_FriendRemove&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommand.friendRemove(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommand_FriendRemoveCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_FriendRemoveCopyWith(UiCommand_FriendRemove value, $Res Function(UiCommand_FriendRemove) _then) = _$UiCommand_FriendRemoveCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommand_FriendRemoveCopyWithImpl<$Res>
    implements $UiCommand_FriendRemoveCopyWith<$Res> {
  _$UiCommand_FriendRemoveCopyWithImpl(this._self, this._then);

  final UiCommand_FriendRemove _self;
  final $Res Function(UiCommand_FriendRemove) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommand_FriendRemove(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_AgentEnqueueTurn extends UiCommand {
  const UiCommand_AgentEnqueueTurn({required this.dest, required this.text, required this.inReplyTo}): super._();
  

 final  String dest;
 final  String text;
 final  PlatformInt64 inReplyTo;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_AgentEnqueueTurnCopyWith<UiCommand_AgentEnqueueTurn> get copyWith => _$UiCommand_AgentEnqueueTurnCopyWithImpl<UiCommand_AgentEnqueueTurn>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_AgentEnqueueTurn&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.text, text) || other.text == text)&&(identical(other.inReplyTo, inReplyTo) || other.inReplyTo == inReplyTo));
}


@override
int get hashCode => Object.hash(runtimeType,dest,text,inReplyTo);

@override
String toString() {
  return 'UiCommand.agentEnqueueTurn(dest: $dest, text: $text, inReplyTo: $inReplyTo)';
}


}

/// @nodoc
abstract mixin class $UiCommand_AgentEnqueueTurnCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_AgentEnqueueTurnCopyWith(UiCommand_AgentEnqueueTurn value, $Res Function(UiCommand_AgentEnqueueTurn) _then) = _$UiCommand_AgentEnqueueTurnCopyWithImpl;
@useResult
$Res call({
 String dest, String text, PlatformInt64 inReplyTo
});




}
/// @nodoc
class _$UiCommand_AgentEnqueueTurnCopyWithImpl<$Res>
    implements $UiCommand_AgentEnqueueTurnCopyWith<$Res> {
  _$UiCommand_AgentEnqueueTurnCopyWithImpl(this._self, this._then);

  final UiCommand_AgentEnqueueTurn _self;
  final $Res Function(UiCommand_AgentEnqueueTurn) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? text = null,Object? inReplyTo = null,}) {
  return _then(UiCommand_AgentEnqueueTurn(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,inReplyTo: null == inReplyTo ? _self.inReplyTo : inReplyTo // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class UiCommand_AgentRespondPermission extends UiCommand {
  const UiCommand_AgentRespondPermission({required this.dest, required this.callId, required this.permission}): super._();
  

 final  String dest;
 final  String callId;
 final  String permission;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_AgentRespondPermissionCopyWith<UiCommand_AgentRespondPermission> get copyWith => _$UiCommand_AgentRespondPermissionCopyWithImpl<UiCommand_AgentRespondPermission>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_AgentRespondPermission&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.callId, callId) || other.callId == callId)&&(identical(other.permission, permission) || other.permission == permission));
}


@override
int get hashCode => Object.hash(runtimeType,dest,callId,permission);

@override
String toString() {
  return 'UiCommand.agentRespondPermission(dest: $dest, callId: $callId, permission: $permission)';
}


}

/// @nodoc
abstract mixin class $UiCommand_AgentRespondPermissionCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_AgentRespondPermissionCopyWith(UiCommand_AgentRespondPermission value, $Res Function(UiCommand_AgentRespondPermission) _then) = _$UiCommand_AgentRespondPermissionCopyWithImpl;
@useResult
$Res call({
 String dest, String callId, String permission
});




}
/// @nodoc
class _$UiCommand_AgentRespondPermissionCopyWithImpl<$Res>
    implements $UiCommand_AgentRespondPermissionCopyWith<$Res> {
  _$UiCommand_AgentRespondPermissionCopyWithImpl(this._self, this._then);

  final UiCommand_AgentRespondPermission _self;
  final $Res Function(UiCommand_AgentRespondPermission) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? callId = null,Object? permission = null,}) {
  return _then(UiCommand_AgentRespondPermission(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,permission: null == permission ? _self.permission : permission // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_AgentAbortTurn extends UiCommand {
  const UiCommand_AgentAbortTurn({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_AgentAbortTurnCopyWith<UiCommand_AgentAbortTurn> get copyWith => _$UiCommand_AgentAbortTurnCopyWithImpl<UiCommand_AgentAbortTurn>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_AgentAbortTurn&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommand.agentAbortTurn(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommand_AgentAbortTurnCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_AgentAbortTurnCopyWith(UiCommand_AgentAbortTurn value, $Res Function(UiCommand_AgentAbortTurn) _then) = _$UiCommand_AgentAbortTurnCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommand_AgentAbortTurnCopyWithImpl<$Res>
    implements $UiCommand_AgentAbortTurnCopyWith<$Res> {
  _$UiCommand_AgentAbortTurnCopyWithImpl(this._self, this._then);

  final UiCommand_AgentAbortTurn _self;
  final $Res Function(UiCommand_AgentAbortTurn) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommand_AgentAbortTurn(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommand_AgentRunResult extends UiCommand {
  const UiCommand_AgentRunResult({required this.dest, required this.profileId, required this.epoch, required this.output, this.error}): super._();
  

 final  String dest;
 final  String profileId;
 final  BigInt epoch;
 final  String output;
 final  String? error;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_AgentRunResultCopyWith<UiCommand_AgentRunResult> get copyWith => _$UiCommand_AgentRunResultCopyWithImpl<UiCommand_AgentRunResult>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_AgentRunResult&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.profileId, profileId) || other.profileId == profileId)&&(identical(other.epoch, epoch) || other.epoch == epoch)&&(identical(other.output, output) || other.output == output)&&(identical(other.error, error) || other.error == error));
}


@override
int get hashCode => Object.hash(runtimeType,dest,profileId,epoch,output,error);

@override
String toString() {
  return 'UiCommand.agentRunResult(dest: $dest, profileId: $profileId, epoch: $epoch, output: $output, error: $error)';
}


}

/// @nodoc
abstract mixin class $UiCommand_AgentRunResultCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_AgentRunResultCopyWith(UiCommand_AgentRunResult value, $Res Function(UiCommand_AgentRunResult) _then) = _$UiCommand_AgentRunResultCopyWithImpl;
@useResult
$Res call({
 String dest, String profileId, BigInt epoch, String output, String? error
});




}
/// @nodoc
class _$UiCommand_AgentRunResultCopyWithImpl<$Res>
    implements $UiCommand_AgentRunResultCopyWith<$Res> {
  _$UiCommand_AgentRunResultCopyWithImpl(this._self, this._then);

  final UiCommand_AgentRunResult _self;
  final $Res Function(UiCommand_AgentRunResult) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? profileId = null,Object? epoch = null,Object? output = null,Object? error = freezed,}) {
  return _then(UiCommand_AgentRunResult(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,profileId: null == profileId ? _self.profileId : profileId // ignore: cast_nullable_to_non_nullable
as String,epoch: null == epoch ? _self.epoch : epoch // ignore: cast_nullable_to_non_nullable
as BigInt,output: null == output ? _self.output : output // ignore: cast_nullable_to_non_nullable
as String,error: freezed == error ? _self.error : error // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class UiCommand_SettingsPatch extends UiCommand {
  const UiCommand_SettingsPatch({this.wsUrl, this.httpOrigin, this.env}): super._();
  

 final  String? wsUrl;
 final  String? httpOrigin;
 final  String? env;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommand_SettingsPatchCopyWith<UiCommand_SettingsPatch> get copyWith => _$UiCommand_SettingsPatchCopyWithImpl<UiCommand_SettingsPatch>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommand_SettingsPatch&&(identical(other.wsUrl, wsUrl) || other.wsUrl == wsUrl)&&(identical(other.httpOrigin, httpOrigin) || other.httpOrigin == httpOrigin)&&(identical(other.env, env) || other.env == env));
}


@override
int get hashCode => Object.hash(runtimeType,wsUrl,httpOrigin,env);

@override
String toString() {
  return 'UiCommand.settingsPatch(wsUrl: $wsUrl, httpOrigin: $httpOrigin, env: $env)';
}


}

/// @nodoc
abstract mixin class $UiCommand_SettingsPatchCopyWith<$Res> implements $UiCommandCopyWith<$Res> {
  factory $UiCommand_SettingsPatchCopyWith(UiCommand_SettingsPatch value, $Res Function(UiCommand_SettingsPatch) _then) = _$UiCommand_SettingsPatchCopyWithImpl;
@useResult
$Res call({
 String? wsUrl, String? httpOrigin, String? env
});




}
/// @nodoc
class _$UiCommand_SettingsPatchCopyWithImpl<$Res>
    implements $UiCommand_SettingsPatchCopyWith<$Res> {
  _$UiCommand_SettingsPatchCopyWithImpl(this._self, this._then);

  final UiCommand_SettingsPatch _self;
  final $Res Function(UiCommand_SettingsPatch) _then;

/// Create a copy of UiCommand
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? wsUrl = freezed,Object? httpOrigin = freezed,Object? env = freezed,}) {
  return _then(UiCommand_SettingsPatch(
wsUrl: freezed == wsUrl ? _self.wsUrl : wsUrl // ignore: cast_nullable_to_non_nullable
as String?,httpOrigin: freezed == httpOrigin ? _self.httpOrigin : httpOrigin // ignore: cast_nullable_to_non_nullable
as String?,env: freezed == env ? _self.env : env // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

// dart format on
