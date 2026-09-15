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
mixin _$LinkStateDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkStateDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LinkStateDto()';
}


}

/// @nodoc
class $LinkStateDtoCopyWith<$Res>  {
$LinkStateDtoCopyWith(LinkStateDto _, $Res Function(LinkStateDto) __);
}


/// Adds pattern-matching-related methods to [LinkStateDto].
extension LinkStateDtoPatterns on LinkStateDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( LinkStateDto_Connecting value)?  connecting,TResult Function( LinkStateDto_Online value)?  online,TResult Function( LinkStateDto_Reconnecting value)?  reconnecting,TResult Function( LinkStateDto_Offline value)?  offline,required TResult orElse(),}){
final _that = this;
switch (_that) {
case LinkStateDto_Connecting() when connecting != null:
return connecting(_that);case LinkStateDto_Online() when online != null:
return online(_that);case LinkStateDto_Reconnecting() when reconnecting != null:
return reconnecting(_that);case LinkStateDto_Offline() when offline != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( LinkStateDto_Connecting value)  connecting,required TResult Function( LinkStateDto_Online value)  online,required TResult Function( LinkStateDto_Reconnecting value)  reconnecting,required TResult Function( LinkStateDto_Offline value)  offline,}){
final _that = this;
switch (_that) {
case LinkStateDto_Connecting():
return connecting(_that);case LinkStateDto_Online():
return online(_that);case LinkStateDto_Reconnecting():
return reconnecting(_that);case LinkStateDto_Offline():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( LinkStateDto_Connecting value)?  connecting,TResult? Function( LinkStateDto_Online value)?  online,TResult? Function( LinkStateDto_Reconnecting value)?  reconnecting,TResult? Function( LinkStateDto_Offline value)?  offline,}){
final _that = this;
switch (_that) {
case LinkStateDto_Connecting() when connecting != null:
return connecting(_that);case LinkStateDto_Online() when online != null:
return online(_that);case LinkStateDto_Reconnecting() when reconnecting != null:
return reconnecting(_that);case LinkStateDto_Offline() when offline != null:
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
case LinkStateDto_Connecting() when connecting != null:
return connecting();case LinkStateDto_Online() when online != null:
return online();case LinkStateDto_Reconnecting() when reconnecting != null:
return reconnecting(_that.attempt);case LinkStateDto_Offline() when offline != null:
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
case LinkStateDto_Connecting():
return connecting();case LinkStateDto_Online():
return online();case LinkStateDto_Reconnecting():
return reconnecting(_that.attempt);case LinkStateDto_Offline():
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
case LinkStateDto_Connecting() when connecting != null:
return connecting();case LinkStateDto_Online() when online != null:
return online();case LinkStateDto_Reconnecting() when reconnecting != null:
return reconnecting(_that.attempt);case LinkStateDto_Offline() when offline != null:
return offline();case _:
  return null;

}
}

}

/// @nodoc


class LinkStateDto_Connecting extends LinkStateDto {
  const LinkStateDto_Connecting(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkStateDto_Connecting);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LinkStateDto.connecting()';
}


}




/// @nodoc


class LinkStateDto_Online extends LinkStateDto {
  const LinkStateDto_Online(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkStateDto_Online);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LinkStateDto.online()';
}


}




/// @nodoc


class LinkStateDto_Reconnecting extends LinkStateDto {
  const LinkStateDto_Reconnecting({required this.attempt}): super._();
  

 final  int attempt;

/// Create a copy of LinkStateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LinkStateDto_ReconnectingCopyWith<LinkStateDto_Reconnecting> get copyWith => _$LinkStateDto_ReconnectingCopyWithImpl<LinkStateDto_Reconnecting>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkStateDto_Reconnecting&&(identical(other.attempt, attempt) || other.attempt == attempt));
}


@override
int get hashCode => Object.hash(runtimeType,attempt);

@override
String toString() {
  return 'LinkStateDto.reconnecting(attempt: $attempt)';
}


}

/// @nodoc
abstract mixin class $LinkStateDto_ReconnectingCopyWith<$Res> implements $LinkStateDtoCopyWith<$Res> {
  factory $LinkStateDto_ReconnectingCopyWith(LinkStateDto_Reconnecting value, $Res Function(LinkStateDto_Reconnecting) _then) = _$LinkStateDto_ReconnectingCopyWithImpl;
@useResult
$Res call({
 int attempt
});




}
/// @nodoc
class _$LinkStateDto_ReconnectingCopyWithImpl<$Res>
    implements $LinkStateDto_ReconnectingCopyWith<$Res> {
  _$LinkStateDto_ReconnectingCopyWithImpl(this._self, this._then);

  final LinkStateDto_Reconnecting _self;
  final $Res Function(LinkStateDto_Reconnecting) _then;

/// Create a copy of LinkStateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? attempt = null,}) {
  return _then(LinkStateDto_Reconnecting(
attempt: null == attempt ? _self.attempt : attempt // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class LinkStateDto_Offline extends LinkStateDto {
  const LinkStateDto_Offline(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkStateDto_Offline);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LinkStateDto.offline()';
}


}




/// @nodoc
mixin _$SessionUpdateDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SessionUpdateDto()';
}


}

/// @nodoc
class $SessionUpdateDtoCopyWith<$Res>  {
$SessionUpdateDtoCopyWith(SessionUpdateDto _, $Res Function(SessionUpdateDto) __);
}


/// Adds pattern-matching-related methods to [SessionUpdateDto].
extension SessionUpdateDtoPatterns on SessionUpdateDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SessionUpdateDto_Link value)?  link,TResult Function( SessionUpdateDto_Inbox value)?  inbox,TResult Function( SessionUpdateDto_ThreadUpsert value)?  threadUpsert,TResult Function( SessionUpdateDto_SyncProgress value)?  syncProgress,TResult Function( SessionUpdateDto_Kickout value)?  kickout,TResult Function( SessionUpdateDto_AuthExpired value)?  authExpired,TResult Function( SessionUpdateDto_TokenRenew value)?  tokenRenew,TResult Function( SessionUpdateDto_FriendRequest value)?  friendRequest,TResult Function( SessionUpdateDto_FriendAccepted value)?  friendAccepted,TResult Function( SessionUpdateDto_ProfileUpdated value)?  profileUpdated,TResult Function( SessionUpdateDto_Presence value)?  presence,TResult Function( SessionUpdateDto_Typing value)?  typing,TResult Function( SessionUpdateDto_ReceiptRead value)?  receiptRead,TResult Function( SessionUpdateDto_GroupCreate value)?  groupCreate,TResult Function( SessionUpdateDto_ContactsChanged value)?  contactsChanged,TResult Function( SessionUpdateDto_AgentTurn value)?  agentTurn,TResult Function( SessionUpdateDto_AgentCard value)?  agentCard,TResult Function( SessionUpdateDto_RustPanic value)?  rustPanic,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SessionUpdateDto_Link() when link != null:
return link(_that);case SessionUpdateDto_Inbox() when inbox != null:
return inbox(_that);case SessionUpdateDto_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that);case SessionUpdateDto_SyncProgress() when syncProgress != null:
return syncProgress(_that);case SessionUpdateDto_Kickout() when kickout != null:
return kickout(_that);case SessionUpdateDto_AuthExpired() when authExpired != null:
return authExpired(_that);case SessionUpdateDto_TokenRenew() when tokenRenew != null:
return tokenRenew(_that);case SessionUpdateDto_FriendRequest() when friendRequest != null:
return friendRequest(_that);case SessionUpdateDto_FriendAccepted() when friendAccepted != null:
return friendAccepted(_that);case SessionUpdateDto_ProfileUpdated() when profileUpdated != null:
return profileUpdated(_that);case SessionUpdateDto_Presence() when presence != null:
return presence(_that);case SessionUpdateDto_Typing() when typing != null:
return typing(_that);case SessionUpdateDto_ReceiptRead() when receiptRead != null:
return receiptRead(_that);case SessionUpdateDto_GroupCreate() when groupCreate != null:
return groupCreate(_that);case SessionUpdateDto_ContactsChanged() when contactsChanged != null:
return contactsChanged(_that);case SessionUpdateDto_AgentTurn() when agentTurn != null:
return agentTurn(_that);case SessionUpdateDto_AgentCard() when agentCard != null:
return agentCard(_that);case SessionUpdateDto_RustPanic() when rustPanic != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SessionUpdateDto_Link value)  link,required TResult Function( SessionUpdateDto_Inbox value)  inbox,required TResult Function( SessionUpdateDto_ThreadUpsert value)  threadUpsert,required TResult Function( SessionUpdateDto_SyncProgress value)  syncProgress,required TResult Function( SessionUpdateDto_Kickout value)  kickout,required TResult Function( SessionUpdateDto_AuthExpired value)  authExpired,required TResult Function( SessionUpdateDto_TokenRenew value)  tokenRenew,required TResult Function( SessionUpdateDto_FriendRequest value)  friendRequest,required TResult Function( SessionUpdateDto_FriendAccepted value)  friendAccepted,required TResult Function( SessionUpdateDto_ProfileUpdated value)  profileUpdated,required TResult Function( SessionUpdateDto_Presence value)  presence,required TResult Function( SessionUpdateDto_Typing value)  typing,required TResult Function( SessionUpdateDto_ReceiptRead value)  receiptRead,required TResult Function( SessionUpdateDto_GroupCreate value)  groupCreate,required TResult Function( SessionUpdateDto_ContactsChanged value)  contactsChanged,required TResult Function( SessionUpdateDto_AgentTurn value)  agentTurn,required TResult Function( SessionUpdateDto_AgentCard value)  agentCard,required TResult Function( SessionUpdateDto_RustPanic value)  rustPanic,}){
final _that = this;
switch (_that) {
case SessionUpdateDto_Link():
return link(_that);case SessionUpdateDto_Inbox():
return inbox(_that);case SessionUpdateDto_ThreadUpsert():
return threadUpsert(_that);case SessionUpdateDto_SyncProgress():
return syncProgress(_that);case SessionUpdateDto_Kickout():
return kickout(_that);case SessionUpdateDto_AuthExpired():
return authExpired(_that);case SessionUpdateDto_TokenRenew():
return tokenRenew(_that);case SessionUpdateDto_FriendRequest():
return friendRequest(_that);case SessionUpdateDto_FriendAccepted():
return friendAccepted(_that);case SessionUpdateDto_ProfileUpdated():
return profileUpdated(_that);case SessionUpdateDto_Presence():
return presence(_that);case SessionUpdateDto_Typing():
return typing(_that);case SessionUpdateDto_ReceiptRead():
return receiptRead(_that);case SessionUpdateDto_GroupCreate():
return groupCreate(_that);case SessionUpdateDto_ContactsChanged():
return contactsChanged(_that);case SessionUpdateDto_AgentTurn():
return agentTurn(_that);case SessionUpdateDto_AgentCard():
return agentCard(_that);case SessionUpdateDto_RustPanic():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SessionUpdateDto_Link value)?  link,TResult? Function( SessionUpdateDto_Inbox value)?  inbox,TResult? Function( SessionUpdateDto_ThreadUpsert value)?  threadUpsert,TResult? Function( SessionUpdateDto_SyncProgress value)?  syncProgress,TResult? Function( SessionUpdateDto_Kickout value)?  kickout,TResult? Function( SessionUpdateDto_AuthExpired value)?  authExpired,TResult? Function( SessionUpdateDto_TokenRenew value)?  tokenRenew,TResult? Function( SessionUpdateDto_FriendRequest value)?  friendRequest,TResult? Function( SessionUpdateDto_FriendAccepted value)?  friendAccepted,TResult? Function( SessionUpdateDto_ProfileUpdated value)?  profileUpdated,TResult? Function( SessionUpdateDto_Presence value)?  presence,TResult? Function( SessionUpdateDto_Typing value)?  typing,TResult? Function( SessionUpdateDto_ReceiptRead value)?  receiptRead,TResult? Function( SessionUpdateDto_GroupCreate value)?  groupCreate,TResult? Function( SessionUpdateDto_ContactsChanged value)?  contactsChanged,TResult? Function( SessionUpdateDto_AgentTurn value)?  agentTurn,TResult? Function( SessionUpdateDto_AgentCard value)?  agentCard,TResult? Function( SessionUpdateDto_RustPanic value)?  rustPanic,}){
final _that = this;
switch (_that) {
case SessionUpdateDto_Link() when link != null:
return link(_that);case SessionUpdateDto_Inbox() when inbox != null:
return inbox(_that);case SessionUpdateDto_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that);case SessionUpdateDto_SyncProgress() when syncProgress != null:
return syncProgress(_that);case SessionUpdateDto_Kickout() when kickout != null:
return kickout(_that);case SessionUpdateDto_AuthExpired() when authExpired != null:
return authExpired(_that);case SessionUpdateDto_TokenRenew() when tokenRenew != null:
return tokenRenew(_that);case SessionUpdateDto_FriendRequest() when friendRequest != null:
return friendRequest(_that);case SessionUpdateDto_FriendAccepted() when friendAccepted != null:
return friendAccepted(_that);case SessionUpdateDto_ProfileUpdated() when profileUpdated != null:
return profileUpdated(_that);case SessionUpdateDto_Presence() when presence != null:
return presence(_that);case SessionUpdateDto_Typing() when typing != null:
return typing(_that);case SessionUpdateDto_ReceiptRead() when receiptRead != null:
return receiptRead(_that);case SessionUpdateDto_GroupCreate() when groupCreate != null:
return groupCreate(_that);case SessionUpdateDto_ContactsChanged() when contactsChanged != null:
return contactsChanged(_that);case SessionUpdateDto_AgentTurn() when agentTurn != null:
return agentTurn(_that);case SessionUpdateDto_AgentCard() when agentCard != null:
return agentCard(_that);case SessionUpdateDto_RustPanic() when rustPanic != null:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( LinkStateDto state,  String? lastError)?  link,TResult Function( List<ThreadViewDto> threads)?  inbox,TResult Function( ThreadViewDto thread)?  threadUpsert,TResult Function( BigInt pulled,  bool catchingUp)?  syncProgress,TResult Function( String channelId)?  kickout,TResult Function( String reason)?  authExpired,TResult Function( String token,  PlatformInt64 exp)?  tokenRenew,TResult Function( String from,  String nickname)?  friendRequest,TResult Function( String from,  String nickname)?  friendAccepted,TResult Function( String account,  String nickname,  String avatar)?  profileUpdated,TResult Function( String account,  int status,  PlatformInt64 lastSeen)?  presence,TResult Function( String typer,  String dest,  int kind,  bool active)?  typing,TResult Function( String reader,  String dest,  int kind,  PlatformInt64 messageId)?  receiptRead,TResult Function( String groupId,  List<String> members)?  groupCreate,TResult Function( List<PersonDto> contacts)?  contactsChanged,TResult Function( String dest,  AgentTurnStateDto state,  String text)?  agentTurn,TResult Function( String dest,  AgentCardDto card)?  agentCard,TResult Function( String message)?  rustPanic,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SessionUpdateDto_Link() when link != null:
return link(_that.state,_that.lastError);case SessionUpdateDto_Inbox() when inbox != null:
return inbox(_that.threads);case SessionUpdateDto_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that.thread);case SessionUpdateDto_SyncProgress() when syncProgress != null:
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdateDto_Kickout() when kickout != null:
return kickout(_that.channelId);case SessionUpdateDto_AuthExpired() when authExpired != null:
return authExpired(_that.reason);case SessionUpdateDto_TokenRenew() when tokenRenew != null:
return tokenRenew(_that.token,_that.exp);case SessionUpdateDto_FriendRequest() when friendRequest != null:
return friendRequest(_that.from,_that.nickname);case SessionUpdateDto_FriendAccepted() when friendAccepted != null:
return friendAccepted(_that.from,_that.nickname);case SessionUpdateDto_ProfileUpdated() when profileUpdated != null:
return profileUpdated(_that.account,_that.nickname,_that.avatar);case SessionUpdateDto_Presence() when presence != null:
return presence(_that.account,_that.status,_that.lastSeen);case SessionUpdateDto_Typing() when typing != null:
return typing(_that.typer,_that.dest,_that.kind,_that.active);case SessionUpdateDto_ReceiptRead() when receiptRead != null:
return receiptRead(_that.reader,_that.dest,_that.kind,_that.messageId);case SessionUpdateDto_GroupCreate() when groupCreate != null:
return groupCreate(_that.groupId,_that.members);case SessionUpdateDto_ContactsChanged() when contactsChanged != null:
return contactsChanged(_that.contacts);case SessionUpdateDto_AgentTurn() when agentTurn != null:
return agentTurn(_that.dest,_that.state,_that.text);case SessionUpdateDto_AgentCard() when agentCard != null:
return agentCard(_that.dest,_that.card);case SessionUpdateDto_RustPanic() when rustPanic != null:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( LinkStateDto state,  String? lastError)  link,required TResult Function( List<ThreadViewDto> threads)  inbox,required TResult Function( ThreadViewDto thread)  threadUpsert,required TResult Function( BigInt pulled,  bool catchingUp)  syncProgress,required TResult Function( String channelId)  kickout,required TResult Function( String reason)  authExpired,required TResult Function( String token,  PlatformInt64 exp)  tokenRenew,required TResult Function( String from,  String nickname)  friendRequest,required TResult Function( String from,  String nickname)  friendAccepted,required TResult Function( String account,  String nickname,  String avatar)  profileUpdated,required TResult Function( String account,  int status,  PlatformInt64 lastSeen)  presence,required TResult Function( String typer,  String dest,  int kind,  bool active)  typing,required TResult Function( String reader,  String dest,  int kind,  PlatformInt64 messageId)  receiptRead,required TResult Function( String groupId,  List<String> members)  groupCreate,required TResult Function( List<PersonDto> contacts)  contactsChanged,required TResult Function( String dest,  AgentTurnStateDto state,  String text)  agentTurn,required TResult Function( String dest,  AgentCardDto card)  agentCard,required TResult Function( String message)  rustPanic,}) {final _that = this;
switch (_that) {
case SessionUpdateDto_Link():
return link(_that.state,_that.lastError);case SessionUpdateDto_Inbox():
return inbox(_that.threads);case SessionUpdateDto_ThreadUpsert():
return threadUpsert(_that.thread);case SessionUpdateDto_SyncProgress():
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdateDto_Kickout():
return kickout(_that.channelId);case SessionUpdateDto_AuthExpired():
return authExpired(_that.reason);case SessionUpdateDto_TokenRenew():
return tokenRenew(_that.token,_that.exp);case SessionUpdateDto_FriendRequest():
return friendRequest(_that.from,_that.nickname);case SessionUpdateDto_FriendAccepted():
return friendAccepted(_that.from,_that.nickname);case SessionUpdateDto_ProfileUpdated():
return profileUpdated(_that.account,_that.nickname,_that.avatar);case SessionUpdateDto_Presence():
return presence(_that.account,_that.status,_that.lastSeen);case SessionUpdateDto_Typing():
return typing(_that.typer,_that.dest,_that.kind,_that.active);case SessionUpdateDto_ReceiptRead():
return receiptRead(_that.reader,_that.dest,_that.kind,_that.messageId);case SessionUpdateDto_GroupCreate():
return groupCreate(_that.groupId,_that.members);case SessionUpdateDto_ContactsChanged():
return contactsChanged(_that.contacts);case SessionUpdateDto_AgentTurn():
return agentTurn(_that.dest,_that.state,_that.text);case SessionUpdateDto_AgentCard():
return agentCard(_that.dest,_that.card);case SessionUpdateDto_RustPanic():
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( LinkStateDto state,  String? lastError)?  link,TResult? Function( List<ThreadViewDto> threads)?  inbox,TResult? Function( ThreadViewDto thread)?  threadUpsert,TResult? Function( BigInt pulled,  bool catchingUp)?  syncProgress,TResult? Function( String channelId)?  kickout,TResult? Function( String reason)?  authExpired,TResult? Function( String token,  PlatformInt64 exp)?  tokenRenew,TResult? Function( String from,  String nickname)?  friendRequest,TResult? Function( String from,  String nickname)?  friendAccepted,TResult? Function( String account,  String nickname,  String avatar)?  profileUpdated,TResult? Function( String account,  int status,  PlatformInt64 lastSeen)?  presence,TResult? Function( String typer,  String dest,  int kind,  bool active)?  typing,TResult? Function( String reader,  String dest,  int kind,  PlatformInt64 messageId)?  receiptRead,TResult? Function( String groupId,  List<String> members)?  groupCreate,TResult? Function( List<PersonDto> contacts)?  contactsChanged,TResult? Function( String dest,  AgentTurnStateDto state,  String text)?  agentTurn,TResult? Function( String dest,  AgentCardDto card)?  agentCard,TResult? Function( String message)?  rustPanic,}) {final _that = this;
switch (_that) {
case SessionUpdateDto_Link() when link != null:
return link(_that.state,_that.lastError);case SessionUpdateDto_Inbox() when inbox != null:
return inbox(_that.threads);case SessionUpdateDto_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that.thread);case SessionUpdateDto_SyncProgress() when syncProgress != null:
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdateDto_Kickout() when kickout != null:
return kickout(_that.channelId);case SessionUpdateDto_AuthExpired() when authExpired != null:
return authExpired(_that.reason);case SessionUpdateDto_TokenRenew() when tokenRenew != null:
return tokenRenew(_that.token,_that.exp);case SessionUpdateDto_FriendRequest() when friendRequest != null:
return friendRequest(_that.from,_that.nickname);case SessionUpdateDto_FriendAccepted() when friendAccepted != null:
return friendAccepted(_that.from,_that.nickname);case SessionUpdateDto_ProfileUpdated() when profileUpdated != null:
return profileUpdated(_that.account,_that.nickname,_that.avatar);case SessionUpdateDto_Presence() when presence != null:
return presence(_that.account,_that.status,_that.lastSeen);case SessionUpdateDto_Typing() when typing != null:
return typing(_that.typer,_that.dest,_that.kind,_that.active);case SessionUpdateDto_ReceiptRead() when receiptRead != null:
return receiptRead(_that.reader,_that.dest,_that.kind,_that.messageId);case SessionUpdateDto_GroupCreate() when groupCreate != null:
return groupCreate(_that.groupId,_that.members);case SessionUpdateDto_ContactsChanged() when contactsChanged != null:
return contactsChanged(_that.contacts);case SessionUpdateDto_AgentTurn() when agentTurn != null:
return agentTurn(_that.dest,_that.state,_that.text);case SessionUpdateDto_AgentCard() when agentCard != null:
return agentCard(_that.dest,_that.card);case SessionUpdateDto_RustPanic() when rustPanic != null:
return rustPanic(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class SessionUpdateDto_Link extends SessionUpdateDto {
  const SessionUpdateDto_Link({required this.state, this.lastError}): super._();
  

 final  LinkStateDto state;
 final  String? lastError;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_LinkCopyWith<SessionUpdateDto_Link> get copyWith => _$SessionUpdateDto_LinkCopyWithImpl<SessionUpdateDto_Link>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_Link&&(identical(other.state, state) || other.state == state)&&(identical(other.lastError, lastError) || other.lastError == lastError));
}


@override
int get hashCode => Object.hash(runtimeType,state,lastError);

@override
String toString() {
  return 'SessionUpdateDto.link(state: $state, lastError: $lastError)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_LinkCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_LinkCopyWith(SessionUpdateDto_Link value, $Res Function(SessionUpdateDto_Link) _then) = _$SessionUpdateDto_LinkCopyWithImpl;
@useResult
$Res call({
 LinkStateDto state, String? lastError
});


$LinkStateDtoCopyWith<$Res> get state;

}
/// @nodoc
class _$SessionUpdateDto_LinkCopyWithImpl<$Res>
    implements $SessionUpdateDto_LinkCopyWith<$Res> {
  _$SessionUpdateDto_LinkCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_Link _self;
  final $Res Function(SessionUpdateDto_Link) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? state = null,Object? lastError = freezed,}) {
  return _then(SessionUpdateDto_Link(
state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as LinkStateDto,lastError: freezed == lastError ? _self.lastError : lastError // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@override
@pragma('vm:prefer-inline')
$LinkStateDtoCopyWith<$Res> get state {
  
  return $LinkStateDtoCopyWith<$Res>(_self.state, (value) {
    return _then(_self.copyWith(state: value));
  });
}
}

/// @nodoc


class SessionUpdateDto_Inbox extends SessionUpdateDto {
  const SessionUpdateDto_Inbox({required  List<ThreadViewDto> threads}): _threads = threads,super._();
  

 final  List<ThreadViewDto> _threads;
 List<ThreadViewDto> get threads {
  if (_threads is EqualUnmodifiableListView) return _threads;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_threads);
}


/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_InboxCopyWith<SessionUpdateDto_Inbox> get copyWith => _$SessionUpdateDto_InboxCopyWithImpl<SessionUpdateDto_Inbox>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_Inbox&&const DeepCollectionEquality().equals(other._threads, _threads));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_threads));

@override
String toString() {
  return 'SessionUpdateDto.inbox(threads: $threads)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_InboxCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_InboxCopyWith(SessionUpdateDto_Inbox value, $Res Function(SessionUpdateDto_Inbox) _then) = _$SessionUpdateDto_InboxCopyWithImpl;
@useResult
$Res call({
 List<ThreadViewDto> threads
});




}
/// @nodoc
class _$SessionUpdateDto_InboxCopyWithImpl<$Res>
    implements $SessionUpdateDto_InboxCopyWith<$Res> {
  _$SessionUpdateDto_InboxCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_Inbox _self;
  final $Res Function(SessionUpdateDto_Inbox) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? threads = null,}) {
  return _then(SessionUpdateDto_Inbox(
threads: null == threads ? _self._threads : threads // ignore: cast_nullable_to_non_nullable
as List<ThreadViewDto>,
  ));
}


}

/// @nodoc


class SessionUpdateDto_ThreadUpsert extends SessionUpdateDto {
  const SessionUpdateDto_ThreadUpsert({required this.thread}): super._();
  

 final  ThreadViewDto thread;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_ThreadUpsertCopyWith<SessionUpdateDto_ThreadUpsert> get copyWith => _$SessionUpdateDto_ThreadUpsertCopyWithImpl<SessionUpdateDto_ThreadUpsert>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_ThreadUpsert&&(identical(other.thread, thread) || other.thread == thread));
}


@override
int get hashCode => Object.hash(runtimeType,thread);

@override
String toString() {
  return 'SessionUpdateDto.threadUpsert(thread: $thread)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_ThreadUpsertCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_ThreadUpsertCopyWith(SessionUpdateDto_ThreadUpsert value, $Res Function(SessionUpdateDto_ThreadUpsert) _then) = _$SessionUpdateDto_ThreadUpsertCopyWithImpl;
@useResult
$Res call({
 ThreadViewDto thread
});




}
/// @nodoc
class _$SessionUpdateDto_ThreadUpsertCopyWithImpl<$Res>
    implements $SessionUpdateDto_ThreadUpsertCopyWith<$Res> {
  _$SessionUpdateDto_ThreadUpsertCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_ThreadUpsert _self;
  final $Res Function(SessionUpdateDto_ThreadUpsert) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? thread = null,}) {
  return _then(SessionUpdateDto_ThreadUpsert(
thread: null == thread ? _self.thread : thread // ignore: cast_nullable_to_non_nullable
as ThreadViewDto,
  ));
}


}

/// @nodoc


class SessionUpdateDto_SyncProgress extends SessionUpdateDto {
  const SessionUpdateDto_SyncProgress({required this.pulled, required this.catchingUp}): super._();
  

 final  BigInt pulled;
 final  bool catchingUp;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_SyncProgressCopyWith<SessionUpdateDto_SyncProgress> get copyWith => _$SessionUpdateDto_SyncProgressCopyWithImpl<SessionUpdateDto_SyncProgress>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_SyncProgress&&(identical(other.pulled, pulled) || other.pulled == pulled)&&(identical(other.catchingUp, catchingUp) || other.catchingUp == catchingUp));
}


@override
int get hashCode => Object.hash(runtimeType,pulled,catchingUp);

@override
String toString() {
  return 'SessionUpdateDto.syncProgress(pulled: $pulled, catchingUp: $catchingUp)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_SyncProgressCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_SyncProgressCopyWith(SessionUpdateDto_SyncProgress value, $Res Function(SessionUpdateDto_SyncProgress) _then) = _$SessionUpdateDto_SyncProgressCopyWithImpl;
@useResult
$Res call({
 BigInt pulled, bool catchingUp
});




}
/// @nodoc
class _$SessionUpdateDto_SyncProgressCopyWithImpl<$Res>
    implements $SessionUpdateDto_SyncProgressCopyWith<$Res> {
  _$SessionUpdateDto_SyncProgressCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_SyncProgress _self;
  final $Res Function(SessionUpdateDto_SyncProgress) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? pulled = null,Object? catchingUp = null,}) {
  return _then(SessionUpdateDto_SyncProgress(
pulled: null == pulled ? _self.pulled : pulled // ignore: cast_nullable_to_non_nullable
as BigInt,catchingUp: null == catchingUp ? _self.catchingUp : catchingUp // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class SessionUpdateDto_Kickout extends SessionUpdateDto {
  const SessionUpdateDto_Kickout({required this.channelId}): super._();
  

 final  String channelId;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_KickoutCopyWith<SessionUpdateDto_Kickout> get copyWith => _$SessionUpdateDto_KickoutCopyWithImpl<SessionUpdateDto_Kickout>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_Kickout&&(identical(other.channelId, channelId) || other.channelId == channelId));
}


@override
int get hashCode => Object.hash(runtimeType,channelId);

@override
String toString() {
  return 'SessionUpdateDto.kickout(channelId: $channelId)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_KickoutCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_KickoutCopyWith(SessionUpdateDto_Kickout value, $Res Function(SessionUpdateDto_Kickout) _then) = _$SessionUpdateDto_KickoutCopyWithImpl;
@useResult
$Res call({
 String channelId
});




}
/// @nodoc
class _$SessionUpdateDto_KickoutCopyWithImpl<$Res>
    implements $SessionUpdateDto_KickoutCopyWith<$Res> {
  _$SessionUpdateDto_KickoutCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_Kickout _self;
  final $Res Function(SessionUpdateDto_Kickout) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? channelId = null,}) {
  return _then(SessionUpdateDto_Kickout(
channelId: null == channelId ? _self.channelId : channelId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdateDto_AuthExpired extends SessionUpdateDto {
  const SessionUpdateDto_AuthExpired({required this.reason}): super._();
  

 final  String reason;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_AuthExpiredCopyWith<SessionUpdateDto_AuthExpired> get copyWith => _$SessionUpdateDto_AuthExpiredCopyWithImpl<SessionUpdateDto_AuthExpired>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_AuthExpired&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,reason);

@override
String toString() {
  return 'SessionUpdateDto.authExpired(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_AuthExpiredCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_AuthExpiredCopyWith(SessionUpdateDto_AuthExpired value, $Res Function(SessionUpdateDto_AuthExpired) _then) = _$SessionUpdateDto_AuthExpiredCopyWithImpl;
@useResult
$Res call({
 String reason
});




}
/// @nodoc
class _$SessionUpdateDto_AuthExpiredCopyWithImpl<$Res>
    implements $SessionUpdateDto_AuthExpiredCopyWith<$Res> {
  _$SessionUpdateDto_AuthExpiredCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_AuthExpired _self;
  final $Res Function(SessionUpdateDto_AuthExpired) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(SessionUpdateDto_AuthExpired(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdateDto_TokenRenew extends SessionUpdateDto {
  const SessionUpdateDto_TokenRenew({required this.token, required this.exp}): super._();
  

 final  String token;
 final  PlatformInt64 exp;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_TokenRenewCopyWith<SessionUpdateDto_TokenRenew> get copyWith => _$SessionUpdateDto_TokenRenewCopyWithImpl<SessionUpdateDto_TokenRenew>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_TokenRenew&&(identical(other.token, token) || other.token == token)&&(identical(other.exp, exp) || other.exp == exp));
}


@override
int get hashCode => Object.hash(runtimeType,token,exp);

@override
String toString() {
  return 'SessionUpdateDto.tokenRenew(token: $token, exp: $exp)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_TokenRenewCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_TokenRenewCopyWith(SessionUpdateDto_TokenRenew value, $Res Function(SessionUpdateDto_TokenRenew) _then) = _$SessionUpdateDto_TokenRenewCopyWithImpl;
@useResult
$Res call({
 String token, PlatformInt64 exp
});




}
/// @nodoc
class _$SessionUpdateDto_TokenRenewCopyWithImpl<$Res>
    implements $SessionUpdateDto_TokenRenewCopyWith<$Res> {
  _$SessionUpdateDto_TokenRenewCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_TokenRenew _self;
  final $Res Function(SessionUpdateDto_TokenRenew) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? token = null,Object? exp = null,}) {
  return _then(SessionUpdateDto_TokenRenew(
token: null == token ? _self.token : token // ignore: cast_nullable_to_non_nullable
as String,exp: null == exp ? _self.exp : exp // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class SessionUpdateDto_FriendRequest extends SessionUpdateDto {
  const SessionUpdateDto_FriendRequest({required this.from, required this.nickname}): super._();
  

 final  String from;
 final  String nickname;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_FriendRequestCopyWith<SessionUpdateDto_FriendRequest> get copyWith => _$SessionUpdateDto_FriendRequestCopyWithImpl<SessionUpdateDto_FriendRequest>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_FriendRequest&&(identical(other.from, from) || other.from == from)&&(identical(other.nickname, nickname) || other.nickname == nickname));
}


@override
int get hashCode => Object.hash(runtimeType,from,nickname);

@override
String toString() {
  return 'SessionUpdateDto.friendRequest(from: $from, nickname: $nickname)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_FriendRequestCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_FriendRequestCopyWith(SessionUpdateDto_FriendRequest value, $Res Function(SessionUpdateDto_FriendRequest) _then) = _$SessionUpdateDto_FriendRequestCopyWithImpl;
@useResult
$Res call({
 String from, String nickname
});




}
/// @nodoc
class _$SessionUpdateDto_FriendRequestCopyWithImpl<$Res>
    implements $SessionUpdateDto_FriendRequestCopyWith<$Res> {
  _$SessionUpdateDto_FriendRequestCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_FriendRequest _self;
  final $Res Function(SessionUpdateDto_FriendRequest) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? from = null,Object? nickname = null,}) {
  return _then(SessionUpdateDto_FriendRequest(
from: null == from ? _self.from : from // ignore: cast_nullable_to_non_nullable
as String,nickname: null == nickname ? _self.nickname : nickname // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdateDto_FriendAccepted extends SessionUpdateDto {
  const SessionUpdateDto_FriendAccepted({required this.from, required this.nickname}): super._();
  

 final  String from;
 final  String nickname;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_FriendAcceptedCopyWith<SessionUpdateDto_FriendAccepted> get copyWith => _$SessionUpdateDto_FriendAcceptedCopyWithImpl<SessionUpdateDto_FriendAccepted>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_FriendAccepted&&(identical(other.from, from) || other.from == from)&&(identical(other.nickname, nickname) || other.nickname == nickname));
}


@override
int get hashCode => Object.hash(runtimeType,from,nickname);

@override
String toString() {
  return 'SessionUpdateDto.friendAccepted(from: $from, nickname: $nickname)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_FriendAcceptedCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_FriendAcceptedCopyWith(SessionUpdateDto_FriendAccepted value, $Res Function(SessionUpdateDto_FriendAccepted) _then) = _$SessionUpdateDto_FriendAcceptedCopyWithImpl;
@useResult
$Res call({
 String from, String nickname
});




}
/// @nodoc
class _$SessionUpdateDto_FriendAcceptedCopyWithImpl<$Res>
    implements $SessionUpdateDto_FriendAcceptedCopyWith<$Res> {
  _$SessionUpdateDto_FriendAcceptedCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_FriendAccepted _self;
  final $Res Function(SessionUpdateDto_FriendAccepted) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? from = null,Object? nickname = null,}) {
  return _then(SessionUpdateDto_FriendAccepted(
from: null == from ? _self.from : from // ignore: cast_nullable_to_non_nullable
as String,nickname: null == nickname ? _self.nickname : nickname // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdateDto_ProfileUpdated extends SessionUpdateDto {
  const SessionUpdateDto_ProfileUpdated({required this.account, required this.nickname, required this.avatar}): super._();
  

 final  String account;
 final  String nickname;
 final  String avatar;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_ProfileUpdatedCopyWith<SessionUpdateDto_ProfileUpdated> get copyWith => _$SessionUpdateDto_ProfileUpdatedCopyWithImpl<SessionUpdateDto_ProfileUpdated>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_ProfileUpdated&&(identical(other.account, account) || other.account == account)&&(identical(other.nickname, nickname) || other.nickname == nickname)&&(identical(other.avatar, avatar) || other.avatar == avatar));
}


@override
int get hashCode => Object.hash(runtimeType,account,nickname,avatar);

@override
String toString() {
  return 'SessionUpdateDto.profileUpdated(account: $account, nickname: $nickname, avatar: $avatar)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_ProfileUpdatedCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_ProfileUpdatedCopyWith(SessionUpdateDto_ProfileUpdated value, $Res Function(SessionUpdateDto_ProfileUpdated) _then) = _$SessionUpdateDto_ProfileUpdatedCopyWithImpl;
@useResult
$Res call({
 String account, String nickname, String avatar
});




}
/// @nodoc
class _$SessionUpdateDto_ProfileUpdatedCopyWithImpl<$Res>
    implements $SessionUpdateDto_ProfileUpdatedCopyWith<$Res> {
  _$SessionUpdateDto_ProfileUpdatedCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_ProfileUpdated _self;
  final $Res Function(SessionUpdateDto_ProfileUpdated) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? account = null,Object? nickname = null,Object? avatar = null,}) {
  return _then(SessionUpdateDto_ProfileUpdated(
account: null == account ? _self.account : account // ignore: cast_nullable_to_non_nullable
as String,nickname: null == nickname ? _self.nickname : nickname // ignore: cast_nullable_to_non_nullable
as String,avatar: null == avatar ? _self.avatar : avatar // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdateDto_Presence extends SessionUpdateDto {
  const SessionUpdateDto_Presence({required this.account, required this.status, required this.lastSeen}): super._();
  

 final  String account;
 final  int status;
 final  PlatformInt64 lastSeen;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_PresenceCopyWith<SessionUpdateDto_Presence> get copyWith => _$SessionUpdateDto_PresenceCopyWithImpl<SessionUpdateDto_Presence>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_Presence&&(identical(other.account, account) || other.account == account)&&(identical(other.status, status) || other.status == status)&&(identical(other.lastSeen, lastSeen) || other.lastSeen == lastSeen));
}


@override
int get hashCode => Object.hash(runtimeType,account,status,lastSeen);

@override
String toString() {
  return 'SessionUpdateDto.presence(account: $account, status: $status, lastSeen: $lastSeen)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_PresenceCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_PresenceCopyWith(SessionUpdateDto_Presence value, $Res Function(SessionUpdateDto_Presence) _then) = _$SessionUpdateDto_PresenceCopyWithImpl;
@useResult
$Res call({
 String account, int status, PlatformInt64 lastSeen
});




}
/// @nodoc
class _$SessionUpdateDto_PresenceCopyWithImpl<$Res>
    implements $SessionUpdateDto_PresenceCopyWith<$Res> {
  _$SessionUpdateDto_PresenceCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_Presence _self;
  final $Res Function(SessionUpdateDto_Presence) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? account = null,Object? status = null,Object? lastSeen = null,}) {
  return _then(SessionUpdateDto_Presence(
account: null == account ? _self.account : account // ignore: cast_nullable_to_non_nullable
as String,status: null == status ? _self.status : status // ignore: cast_nullable_to_non_nullable
as int,lastSeen: null == lastSeen ? _self.lastSeen : lastSeen // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class SessionUpdateDto_Typing extends SessionUpdateDto {
  const SessionUpdateDto_Typing({required this.typer, required this.dest, required this.kind, required this.active}): super._();
  

 final  String typer;
 final  String dest;
 final  int kind;
 final  bool active;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_TypingCopyWith<SessionUpdateDto_Typing> get copyWith => _$SessionUpdateDto_TypingCopyWithImpl<SessionUpdateDto_Typing>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_Typing&&(identical(other.typer, typer) || other.typer == typer)&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.active, active) || other.active == active));
}


@override
int get hashCode => Object.hash(runtimeType,typer,dest,kind,active);

@override
String toString() {
  return 'SessionUpdateDto.typing(typer: $typer, dest: $dest, kind: $kind, active: $active)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_TypingCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_TypingCopyWith(SessionUpdateDto_Typing value, $Res Function(SessionUpdateDto_Typing) _then) = _$SessionUpdateDto_TypingCopyWithImpl;
@useResult
$Res call({
 String typer, String dest, int kind, bool active
});




}
/// @nodoc
class _$SessionUpdateDto_TypingCopyWithImpl<$Res>
    implements $SessionUpdateDto_TypingCopyWith<$Res> {
  _$SessionUpdateDto_TypingCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_Typing _self;
  final $Res Function(SessionUpdateDto_Typing) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? typer = null,Object? dest = null,Object? kind = null,Object? active = null,}) {
  return _then(SessionUpdateDto_Typing(
typer: null == typer ? _self.typer : typer // ignore: cast_nullable_to_non_nullable
as String,dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,active: null == active ? _self.active : active // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class SessionUpdateDto_ReceiptRead extends SessionUpdateDto {
  const SessionUpdateDto_ReceiptRead({required this.reader, required this.dest, required this.kind, required this.messageId}): super._();
  

 final  String reader;
 final  String dest;
 final  int kind;
 final  PlatformInt64 messageId;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_ReceiptReadCopyWith<SessionUpdateDto_ReceiptRead> get copyWith => _$SessionUpdateDto_ReceiptReadCopyWithImpl<SessionUpdateDto_ReceiptRead>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_ReceiptRead&&(identical(other.reader, reader) || other.reader == reader)&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.messageId, messageId) || other.messageId == messageId));
}


@override
int get hashCode => Object.hash(runtimeType,reader,dest,kind,messageId);

@override
String toString() {
  return 'SessionUpdateDto.receiptRead(reader: $reader, dest: $dest, kind: $kind, messageId: $messageId)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_ReceiptReadCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_ReceiptReadCopyWith(SessionUpdateDto_ReceiptRead value, $Res Function(SessionUpdateDto_ReceiptRead) _then) = _$SessionUpdateDto_ReceiptReadCopyWithImpl;
@useResult
$Res call({
 String reader, String dest, int kind, PlatformInt64 messageId
});




}
/// @nodoc
class _$SessionUpdateDto_ReceiptReadCopyWithImpl<$Res>
    implements $SessionUpdateDto_ReceiptReadCopyWith<$Res> {
  _$SessionUpdateDto_ReceiptReadCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_ReceiptRead _self;
  final $Res Function(SessionUpdateDto_ReceiptRead) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reader = null,Object? dest = null,Object? kind = null,Object? messageId = null,}) {
  return _then(SessionUpdateDto_ReceiptRead(
reader: null == reader ? _self.reader : reader // ignore: cast_nullable_to_non_nullable
as String,dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,messageId: null == messageId ? _self.messageId : messageId // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class SessionUpdateDto_GroupCreate extends SessionUpdateDto {
  const SessionUpdateDto_GroupCreate({required this.groupId, required  List<String> members}): _members = members,super._();
  

 final  String groupId;
 final  List<String> _members;
 List<String> get members {
  if (_members is EqualUnmodifiableListView) return _members;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_members);
}


/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_GroupCreateCopyWith<SessionUpdateDto_GroupCreate> get copyWith => _$SessionUpdateDto_GroupCreateCopyWithImpl<SessionUpdateDto_GroupCreate>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_GroupCreate&&(identical(other.groupId, groupId) || other.groupId == groupId)&&const DeepCollectionEquality().equals(other._members, _members));
}


@override
int get hashCode => Object.hash(runtimeType,groupId,const DeepCollectionEquality().hash(_members));

@override
String toString() {
  return 'SessionUpdateDto.groupCreate(groupId: $groupId, members: $members)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_GroupCreateCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_GroupCreateCopyWith(SessionUpdateDto_GroupCreate value, $Res Function(SessionUpdateDto_GroupCreate) _then) = _$SessionUpdateDto_GroupCreateCopyWithImpl;
@useResult
$Res call({
 String groupId, List<String> members
});




}
/// @nodoc
class _$SessionUpdateDto_GroupCreateCopyWithImpl<$Res>
    implements $SessionUpdateDto_GroupCreateCopyWith<$Res> {
  _$SessionUpdateDto_GroupCreateCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_GroupCreate _self;
  final $Res Function(SessionUpdateDto_GroupCreate) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? groupId = null,Object? members = null,}) {
  return _then(SessionUpdateDto_GroupCreate(
groupId: null == groupId ? _self.groupId : groupId // ignore: cast_nullable_to_non_nullable
as String,members: null == members ? _self._members : members // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class SessionUpdateDto_ContactsChanged extends SessionUpdateDto {
  const SessionUpdateDto_ContactsChanged({required  List<PersonDto> contacts}): _contacts = contacts,super._();
  

 final  List<PersonDto> _contacts;
 List<PersonDto> get contacts {
  if (_contacts is EqualUnmodifiableListView) return _contacts;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_contacts);
}


/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_ContactsChangedCopyWith<SessionUpdateDto_ContactsChanged> get copyWith => _$SessionUpdateDto_ContactsChangedCopyWithImpl<SessionUpdateDto_ContactsChanged>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_ContactsChanged&&const DeepCollectionEquality().equals(other._contacts, _contacts));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_contacts));

@override
String toString() {
  return 'SessionUpdateDto.contactsChanged(contacts: $contacts)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_ContactsChangedCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_ContactsChangedCopyWith(SessionUpdateDto_ContactsChanged value, $Res Function(SessionUpdateDto_ContactsChanged) _then) = _$SessionUpdateDto_ContactsChangedCopyWithImpl;
@useResult
$Res call({
 List<PersonDto> contacts
});




}
/// @nodoc
class _$SessionUpdateDto_ContactsChangedCopyWithImpl<$Res>
    implements $SessionUpdateDto_ContactsChangedCopyWith<$Res> {
  _$SessionUpdateDto_ContactsChangedCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_ContactsChanged _self;
  final $Res Function(SessionUpdateDto_ContactsChanged) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? contacts = null,}) {
  return _then(SessionUpdateDto_ContactsChanged(
contacts: null == contacts ? _self._contacts : contacts // ignore: cast_nullable_to_non_nullable
as List<PersonDto>,
  ));
}


}

/// @nodoc


class SessionUpdateDto_AgentTurn extends SessionUpdateDto {
  const SessionUpdateDto_AgentTurn({required this.dest, required this.state, required this.text}): super._();
  

 final  String dest;
 final  AgentTurnStateDto state;
 final  String text;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_AgentTurnCopyWith<SessionUpdateDto_AgentTurn> get copyWith => _$SessionUpdateDto_AgentTurnCopyWithImpl<SessionUpdateDto_AgentTurn>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_AgentTurn&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.state, state) || other.state == state)&&(identical(other.text, text) || other.text == text));
}


@override
int get hashCode => Object.hash(runtimeType,dest,state,text);

@override
String toString() {
  return 'SessionUpdateDto.agentTurn(dest: $dest, state: $state, text: $text)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_AgentTurnCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_AgentTurnCopyWith(SessionUpdateDto_AgentTurn value, $Res Function(SessionUpdateDto_AgentTurn) _then) = _$SessionUpdateDto_AgentTurnCopyWithImpl;
@useResult
$Res call({
 String dest, AgentTurnStateDto state, String text
});




}
/// @nodoc
class _$SessionUpdateDto_AgentTurnCopyWithImpl<$Res>
    implements $SessionUpdateDto_AgentTurnCopyWith<$Res> {
  _$SessionUpdateDto_AgentTurnCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_AgentTurn _self;
  final $Res Function(SessionUpdateDto_AgentTurn) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? state = null,Object? text = null,}) {
  return _then(SessionUpdateDto_AgentTurn(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as AgentTurnStateDto,text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SessionUpdateDto_AgentCard extends SessionUpdateDto {
  const SessionUpdateDto_AgentCard({required this.dest, required this.card}): super._();
  

 final  String dest;
 final  AgentCardDto card;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_AgentCardCopyWith<SessionUpdateDto_AgentCard> get copyWith => _$SessionUpdateDto_AgentCardCopyWithImpl<SessionUpdateDto_AgentCard>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_AgentCard&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.card, card) || other.card == card));
}


@override
int get hashCode => Object.hash(runtimeType,dest,card);

@override
String toString() {
  return 'SessionUpdateDto.agentCard(dest: $dest, card: $card)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_AgentCardCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_AgentCardCopyWith(SessionUpdateDto_AgentCard value, $Res Function(SessionUpdateDto_AgentCard) _then) = _$SessionUpdateDto_AgentCardCopyWithImpl;
@useResult
$Res call({
 String dest, AgentCardDto card
});




}
/// @nodoc
class _$SessionUpdateDto_AgentCardCopyWithImpl<$Res>
    implements $SessionUpdateDto_AgentCardCopyWith<$Res> {
  _$SessionUpdateDto_AgentCardCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_AgentCard _self;
  final $Res Function(SessionUpdateDto_AgentCard) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? card = null,}) {
  return _then(SessionUpdateDto_AgentCard(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,card: null == card ? _self.card : card // ignore: cast_nullable_to_non_nullable
as AgentCardDto,
  ));
}


}

/// @nodoc


class SessionUpdateDto_RustPanic extends SessionUpdateDto {
  const SessionUpdateDto_RustPanic({required this.message}): super._();
  

 final  String message;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SessionUpdateDto_RustPanicCopyWith<SessionUpdateDto_RustPanic> get copyWith => _$SessionUpdateDto_RustPanicCopyWithImpl<SessionUpdateDto_RustPanic>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SessionUpdateDto_RustPanic&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'SessionUpdateDto.rustPanic(message: $message)';
}


}

/// @nodoc
abstract mixin class $SessionUpdateDto_RustPanicCopyWith<$Res> implements $SessionUpdateDtoCopyWith<$Res> {
  factory $SessionUpdateDto_RustPanicCopyWith(SessionUpdateDto_RustPanic value, $Res Function(SessionUpdateDto_RustPanic) _then) = _$SessionUpdateDto_RustPanicCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$SessionUpdateDto_RustPanicCopyWithImpl<$Res>
    implements $SessionUpdateDto_RustPanicCopyWith<$Res> {
  _$SessionUpdateDto_RustPanicCopyWithImpl(this._self, this._then);

  final SessionUpdateDto_RustPanic _self;
  final $Res Function(SessionUpdateDto_RustPanic) _then;

/// Create a copy of SessionUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(SessionUpdateDto_RustPanic(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$TimelineUpdateDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TimelineUpdateDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'TimelineUpdateDto()';
}


}

/// @nodoc
class $TimelineUpdateDtoCopyWith<$Res>  {
$TimelineUpdateDtoCopyWith(TimelineUpdateDto _, $Res Function(TimelineUpdateDto) __);
}


/// Adds pattern-matching-related methods to [TimelineUpdateDto].
extension TimelineUpdateDtoPatterns on TimelineUpdateDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( TimelineUpdateDto_Snapshot value)?  snapshot,TResult Function( TimelineUpdateDto_Delta value)?  delta,TResult Function( TimelineUpdateDto_Resync value)?  resync,required TResult orElse(),}){
final _that = this;
switch (_that) {
case TimelineUpdateDto_Snapshot() when snapshot != null:
return snapshot(_that);case TimelineUpdateDto_Delta() when delta != null:
return delta(_that);case TimelineUpdateDto_Resync() when resync != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( TimelineUpdateDto_Snapshot value)  snapshot,required TResult Function( TimelineUpdateDto_Delta value)  delta,required TResult Function( TimelineUpdateDto_Resync value)  resync,}){
final _that = this;
switch (_that) {
case TimelineUpdateDto_Snapshot():
return snapshot(_that);case TimelineUpdateDto_Delta():
return delta(_that);case TimelineUpdateDto_Resync():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( TimelineUpdateDto_Snapshot value)?  snapshot,TResult? Function( TimelineUpdateDto_Delta value)?  delta,TResult? Function( TimelineUpdateDto_Resync value)?  resync,}){
final _that = this;
switch (_that) {
case TimelineUpdateDto_Snapshot() when snapshot != null:
return snapshot(_that);case TimelineUpdateDto_Delta() when delta != null:
return delta(_that);case TimelineUpdateDto_Resync() when resync != null:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( TimelineSnapshotDto snapshot)?  snapshot,TResult Function( TimelineDeltaDto delta)?  delta,TResult Function( String dest,  String reason)?  resync,required TResult orElse(),}) {final _that = this;
switch (_that) {
case TimelineUpdateDto_Snapshot() when snapshot != null:
return snapshot(_that.snapshot);case TimelineUpdateDto_Delta() when delta != null:
return delta(_that.delta);case TimelineUpdateDto_Resync() when resync != null:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( TimelineSnapshotDto snapshot)  snapshot,required TResult Function( TimelineDeltaDto delta)  delta,required TResult Function( String dest,  String reason)  resync,}) {final _that = this;
switch (_that) {
case TimelineUpdateDto_Snapshot():
return snapshot(_that.snapshot);case TimelineUpdateDto_Delta():
return delta(_that.delta);case TimelineUpdateDto_Resync():
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( TimelineSnapshotDto snapshot)?  snapshot,TResult? Function( TimelineDeltaDto delta)?  delta,TResult? Function( String dest,  String reason)?  resync,}) {final _that = this;
switch (_that) {
case TimelineUpdateDto_Snapshot() when snapshot != null:
return snapshot(_that.snapshot);case TimelineUpdateDto_Delta() when delta != null:
return delta(_that.delta);case TimelineUpdateDto_Resync() when resync != null:
return resync(_that.dest,_that.reason);case _:
  return null;

}
}

}

/// @nodoc


class TimelineUpdateDto_Snapshot extends TimelineUpdateDto {
  const TimelineUpdateDto_Snapshot({required this.snapshot}): super._();
  

 final  TimelineSnapshotDto snapshot;

/// Create a copy of TimelineUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TimelineUpdateDto_SnapshotCopyWith<TimelineUpdateDto_Snapshot> get copyWith => _$TimelineUpdateDto_SnapshotCopyWithImpl<TimelineUpdateDto_Snapshot>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TimelineUpdateDto_Snapshot&&(identical(other.snapshot, snapshot) || other.snapshot == snapshot));
}


@override
int get hashCode => Object.hash(runtimeType,snapshot);

@override
String toString() {
  return 'TimelineUpdateDto.snapshot(snapshot: $snapshot)';
}


}

/// @nodoc
abstract mixin class $TimelineUpdateDto_SnapshotCopyWith<$Res> implements $TimelineUpdateDtoCopyWith<$Res> {
  factory $TimelineUpdateDto_SnapshotCopyWith(TimelineUpdateDto_Snapshot value, $Res Function(TimelineUpdateDto_Snapshot) _then) = _$TimelineUpdateDto_SnapshotCopyWithImpl;
@useResult
$Res call({
 TimelineSnapshotDto snapshot
});




}
/// @nodoc
class _$TimelineUpdateDto_SnapshotCopyWithImpl<$Res>
    implements $TimelineUpdateDto_SnapshotCopyWith<$Res> {
  _$TimelineUpdateDto_SnapshotCopyWithImpl(this._self, this._then);

  final TimelineUpdateDto_Snapshot _self;
  final $Res Function(TimelineUpdateDto_Snapshot) _then;

/// Create a copy of TimelineUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? snapshot = null,}) {
  return _then(TimelineUpdateDto_Snapshot(
snapshot: null == snapshot ? _self.snapshot : snapshot // ignore: cast_nullable_to_non_nullable
as TimelineSnapshotDto,
  ));
}


}

/// @nodoc


class TimelineUpdateDto_Delta extends TimelineUpdateDto {
  const TimelineUpdateDto_Delta({required this.delta}): super._();
  

 final  TimelineDeltaDto delta;

/// Create a copy of TimelineUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TimelineUpdateDto_DeltaCopyWith<TimelineUpdateDto_Delta> get copyWith => _$TimelineUpdateDto_DeltaCopyWithImpl<TimelineUpdateDto_Delta>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TimelineUpdateDto_Delta&&(identical(other.delta, delta) || other.delta == delta));
}


@override
int get hashCode => Object.hash(runtimeType,delta);

@override
String toString() {
  return 'TimelineUpdateDto.delta(delta: $delta)';
}


}

/// @nodoc
abstract mixin class $TimelineUpdateDto_DeltaCopyWith<$Res> implements $TimelineUpdateDtoCopyWith<$Res> {
  factory $TimelineUpdateDto_DeltaCopyWith(TimelineUpdateDto_Delta value, $Res Function(TimelineUpdateDto_Delta) _then) = _$TimelineUpdateDto_DeltaCopyWithImpl;
@useResult
$Res call({
 TimelineDeltaDto delta
});




}
/// @nodoc
class _$TimelineUpdateDto_DeltaCopyWithImpl<$Res>
    implements $TimelineUpdateDto_DeltaCopyWith<$Res> {
  _$TimelineUpdateDto_DeltaCopyWithImpl(this._self, this._then);

  final TimelineUpdateDto_Delta _self;
  final $Res Function(TimelineUpdateDto_Delta) _then;

/// Create a copy of TimelineUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? delta = null,}) {
  return _then(TimelineUpdateDto_Delta(
delta: null == delta ? _self.delta : delta // ignore: cast_nullable_to_non_nullable
as TimelineDeltaDto,
  ));
}


}

/// @nodoc


class TimelineUpdateDto_Resync extends TimelineUpdateDto {
  const TimelineUpdateDto_Resync({required this.dest, required this.reason}): super._();
  

 final  String dest;
 final  String reason;

/// Create a copy of TimelineUpdateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TimelineUpdateDto_ResyncCopyWith<TimelineUpdateDto_Resync> get copyWith => _$TimelineUpdateDto_ResyncCopyWithImpl<TimelineUpdateDto_Resync>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TimelineUpdateDto_Resync&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,dest,reason);

@override
String toString() {
  return 'TimelineUpdateDto.resync(dest: $dest, reason: $reason)';
}


}

/// @nodoc
abstract mixin class $TimelineUpdateDto_ResyncCopyWith<$Res> implements $TimelineUpdateDtoCopyWith<$Res> {
  factory $TimelineUpdateDto_ResyncCopyWith(TimelineUpdateDto_Resync value, $Res Function(TimelineUpdateDto_Resync) _then) = _$TimelineUpdateDto_ResyncCopyWithImpl;
@useResult
$Res call({
 String dest, String reason
});




}
/// @nodoc
class _$TimelineUpdateDto_ResyncCopyWithImpl<$Res>
    implements $TimelineUpdateDto_ResyncCopyWith<$Res> {
  _$TimelineUpdateDto_ResyncCopyWithImpl(this._self, this._then);

  final TimelineUpdateDto_Resync _self;
  final $Res Function(TimelineUpdateDto_Resync) _then;

/// Create a copy of TimelineUpdateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? reason = null,}) {
  return _then(TimelineUpdateDto_Resync(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$TokenPersistDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TokenPersistDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'TokenPersistDto()';
}


}

/// @nodoc
class $TokenPersistDtoCopyWith<$Res>  {
$TokenPersistDtoCopyWith(TokenPersistDto _, $Res Function(TokenPersistDto) __);
}


/// Adds pattern-matching-related methods to [TokenPersistDto].
extension TokenPersistDtoPatterns on TokenPersistDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( TokenPersistDto_Write value)?  write,TResult Function( TokenPersistDto_Clear value)?  clear,required TResult orElse(),}){
final _that = this;
switch (_that) {
case TokenPersistDto_Write() when write != null:
return write(_that);case TokenPersistDto_Clear() when clear != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( TokenPersistDto_Write value)  write,required TResult Function( TokenPersistDto_Clear value)  clear,}){
final _that = this;
switch (_that) {
case TokenPersistDto_Write():
return write(_that);case TokenPersistDto_Clear():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( TokenPersistDto_Write value)?  write,TResult? Function( TokenPersistDto_Clear value)?  clear,}){
final _that = this;
switch (_that) {
case TokenPersistDto_Write() when write != null:
return write(_that);case TokenPersistDto_Clear() when clear != null:
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
case TokenPersistDto_Write() when write != null:
return write(_that.token);case TokenPersistDto_Clear() when clear != null:
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
case TokenPersistDto_Write():
return write(_that.token);case TokenPersistDto_Clear():
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
case TokenPersistDto_Write() when write != null:
return write(_that.token);case TokenPersistDto_Clear() when clear != null:
return clear();case _:
  return null;

}
}

}

/// @nodoc


class TokenPersistDto_Write extends TokenPersistDto {
  const TokenPersistDto_Write({required this.token}): super._();
  

 final  String token;

/// Create a copy of TokenPersistDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TokenPersistDto_WriteCopyWith<TokenPersistDto_Write> get copyWith => _$TokenPersistDto_WriteCopyWithImpl<TokenPersistDto_Write>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TokenPersistDto_Write&&(identical(other.token, token) || other.token == token));
}


@override
int get hashCode => Object.hash(runtimeType,token);

@override
String toString() {
  return 'TokenPersistDto.write(token: $token)';
}


}

/// @nodoc
abstract mixin class $TokenPersistDto_WriteCopyWith<$Res> implements $TokenPersistDtoCopyWith<$Res> {
  factory $TokenPersistDto_WriteCopyWith(TokenPersistDto_Write value, $Res Function(TokenPersistDto_Write) _then) = _$TokenPersistDto_WriteCopyWithImpl;
@useResult
$Res call({
 String token
});




}
/// @nodoc
class _$TokenPersistDto_WriteCopyWithImpl<$Res>
    implements $TokenPersistDto_WriteCopyWith<$Res> {
  _$TokenPersistDto_WriteCopyWithImpl(this._self, this._then);

  final TokenPersistDto_Write _self;
  final $Res Function(TokenPersistDto_Write) _then;

/// Create a copy of TokenPersistDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? token = null,}) {
  return _then(TokenPersistDto_Write(
token: null == token ? _self.token : token // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class TokenPersistDto_Clear extends TokenPersistDto {
  const TokenPersistDto_Clear(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is TokenPersistDto_Clear);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'TokenPersistDto.clear()';
}


}




/// @nodoc
mixin _$UiCommandDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'UiCommandDto()';
}


}

/// @nodoc
class $UiCommandDtoCopyWith<$Res>  {
$UiCommandDtoCopyWith(UiCommandDto _, $Res Function(UiCommandDto) __);
}


/// Adds pattern-matching-related methods to [UiCommandDto].
extension UiCommandDtoPatterns on UiCommandDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( UiCommandDto_SendText value)?  sendText,TResult Function( UiCommandDto_SendMedia value)?  sendMedia,TResult Function( UiCommandDto_RetrySend value)?  retrySend,TResult Function( UiCommandDto_CancelSend value)?  cancelSend,TResult Function( UiCommandDto_MarkThreadRead value)?  markThreadRead,TResult Function( UiCommandDto_DeleteThread value)?  deleteThread,TResult Function( UiCommandDto_FriendRequest value)?  friendRequest,TResult Function( UiCommandDto_FriendAccept value)?  friendAccept,TResult Function( UiCommandDto_FriendReject value)?  friendReject,TResult Function( UiCommandDto_FriendRemove value)?  friendRemove,TResult Function( UiCommandDto_AgentEnqueueTurn value)?  agentEnqueueTurn,TResult Function( UiCommandDto_AgentRespondPermission value)?  agentRespondPermission,TResult Function( UiCommandDto_AgentAbortTurn value)?  agentAbortTurn,TResult Function( UiCommandDto_AgentRunResult value)?  agentRunResult,TResult Function( UiCommandDto_SettingsPatch value)?  settingsPatch,required TResult orElse(),}){
final _that = this;
switch (_that) {
case UiCommandDto_SendText() when sendText != null:
return sendText(_that);case UiCommandDto_SendMedia() when sendMedia != null:
return sendMedia(_that);case UiCommandDto_RetrySend() when retrySend != null:
return retrySend(_that);case UiCommandDto_CancelSend() when cancelSend != null:
return cancelSend(_that);case UiCommandDto_MarkThreadRead() when markThreadRead != null:
return markThreadRead(_that);case UiCommandDto_DeleteThread() when deleteThread != null:
return deleteThread(_that);case UiCommandDto_FriendRequest() when friendRequest != null:
return friendRequest(_that);case UiCommandDto_FriendAccept() when friendAccept != null:
return friendAccept(_that);case UiCommandDto_FriendReject() when friendReject != null:
return friendReject(_that);case UiCommandDto_FriendRemove() when friendRemove != null:
return friendRemove(_that);case UiCommandDto_AgentEnqueueTurn() when agentEnqueueTurn != null:
return agentEnqueueTurn(_that);case UiCommandDto_AgentRespondPermission() when agentRespondPermission != null:
return agentRespondPermission(_that);case UiCommandDto_AgentAbortTurn() when agentAbortTurn != null:
return agentAbortTurn(_that);case UiCommandDto_AgentRunResult() when agentRunResult != null:
return agentRunResult(_that);case UiCommandDto_SettingsPatch() when settingsPatch != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( UiCommandDto_SendText value)  sendText,required TResult Function( UiCommandDto_SendMedia value)  sendMedia,required TResult Function( UiCommandDto_RetrySend value)  retrySend,required TResult Function( UiCommandDto_CancelSend value)  cancelSend,required TResult Function( UiCommandDto_MarkThreadRead value)  markThreadRead,required TResult Function( UiCommandDto_DeleteThread value)  deleteThread,required TResult Function( UiCommandDto_FriendRequest value)  friendRequest,required TResult Function( UiCommandDto_FriendAccept value)  friendAccept,required TResult Function( UiCommandDto_FriendReject value)  friendReject,required TResult Function( UiCommandDto_FriendRemove value)  friendRemove,required TResult Function( UiCommandDto_AgentEnqueueTurn value)  agentEnqueueTurn,required TResult Function( UiCommandDto_AgentRespondPermission value)  agentRespondPermission,required TResult Function( UiCommandDto_AgentAbortTurn value)  agentAbortTurn,required TResult Function( UiCommandDto_AgentRunResult value)  agentRunResult,required TResult Function( UiCommandDto_SettingsPatch value)  settingsPatch,}){
final _that = this;
switch (_that) {
case UiCommandDto_SendText():
return sendText(_that);case UiCommandDto_SendMedia():
return sendMedia(_that);case UiCommandDto_RetrySend():
return retrySend(_that);case UiCommandDto_CancelSend():
return cancelSend(_that);case UiCommandDto_MarkThreadRead():
return markThreadRead(_that);case UiCommandDto_DeleteThread():
return deleteThread(_that);case UiCommandDto_FriendRequest():
return friendRequest(_that);case UiCommandDto_FriendAccept():
return friendAccept(_that);case UiCommandDto_FriendReject():
return friendReject(_that);case UiCommandDto_FriendRemove():
return friendRemove(_that);case UiCommandDto_AgentEnqueueTurn():
return agentEnqueueTurn(_that);case UiCommandDto_AgentRespondPermission():
return agentRespondPermission(_that);case UiCommandDto_AgentAbortTurn():
return agentAbortTurn(_that);case UiCommandDto_AgentRunResult():
return agentRunResult(_that);case UiCommandDto_SettingsPatch():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( UiCommandDto_SendText value)?  sendText,TResult? Function( UiCommandDto_SendMedia value)?  sendMedia,TResult? Function( UiCommandDto_RetrySend value)?  retrySend,TResult? Function( UiCommandDto_CancelSend value)?  cancelSend,TResult? Function( UiCommandDto_MarkThreadRead value)?  markThreadRead,TResult? Function( UiCommandDto_DeleteThread value)?  deleteThread,TResult? Function( UiCommandDto_FriendRequest value)?  friendRequest,TResult? Function( UiCommandDto_FriendAccept value)?  friendAccept,TResult? Function( UiCommandDto_FriendReject value)?  friendReject,TResult? Function( UiCommandDto_FriendRemove value)?  friendRemove,TResult? Function( UiCommandDto_AgentEnqueueTurn value)?  agentEnqueueTurn,TResult? Function( UiCommandDto_AgentRespondPermission value)?  agentRespondPermission,TResult? Function( UiCommandDto_AgentAbortTurn value)?  agentAbortTurn,TResult? Function( UiCommandDto_AgentRunResult value)?  agentRunResult,TResult? Function( UiCommandDto_SettingsPatch value)?  settingsPatch,}){
final _that = this;
switch (_that) {
case UiCommandDto_SendText() when sendText != null:
return sendText(_that);case UiCommandDto_SendMedia() when sendMedia != null:
return sendMedia(_that);case UiCommandDto_RetrySend() when retrySend != null:
return retrySend(_that);case UiCommandDto_CancelSend() when cancelSend != null:
return cancelSend(_that);case UiCommandDto_MarkThreadRead() when markThreadRead != null:
return markThreadRead(_that);case UiCommandDto_DeleteThread() when deleteThread != null:
return deleteThread(_that);case UiCommandDto_FriendRequest() when friendRequest != null:
return friendRequest(_that);case UiCommandDto_FriendAccept() when friendAccept != null:
return friendAccept(_that);case UiCommandDto_FriendReject() when friendReject != null:
return friendReject(_that);case UiCommandDto_FriendRemove() when friendRemove != null:
return friendRemove(_that);case UiCommandDto_AgentEnqueueTurn() when agentEnqueueTurn != null:
return agentEnqueueTurn(_that);case UiCommandDto_AgentRespondPermission() when agentRespondPermission != null:
return agentRespondPermission(_that);case UiCommandDto_AgentAbortTurn() when agentAbortTurn != null:
return agentAbortTurn(_that);case UiCommandDto_AgentRunResult() when agentRunResult != null:
return agentRunResult(_that);case UiCommandDto_SettingsPatch() when settingsPatch != null:
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
case UiCommandDto_SendText() when sendText != null:
return sendText(_that.dest,_that.text,_that.kind);case UiCommandDto_SendMedia() when sendMedia != null:
return sendMedia(_that.dest,_that.path,_that.mime,_that.width,_that.height,_that.byteSize,_that.kind);case UiCommandDto_RetrySend() when retrySend != null:
return retrySend(_that.clientId);case UiCommandDto_CancelSend() when cancelSend != null:
return cancelSend(_that.clientId);case UiCommandDto_MarkThreadRead() when markThreadRead != null:
return markThreadRead(_that.dest,_that.kind,_that.visibleMessageId);case UiCommandDto_DeleteThread() when deleteThread != null:
return deleteThread(_that.dest);case UiCommandDto_FriendRequest() when friendRequest != null:
return friendRequest(_that.dest);case UiCommandDto_FriendAccept() when friendAccept != null:
return friendAccept(_that.dest);case UiCommandDto_FriendReject() when friendReject != null:
return friendReject(_that.dest);case UiCommandDto_FriendRemove() when friendRemove != null:
return friendRemove(_that.dest);case UiCommandDto_AgentEnqueueTurn() when agentEnqueueTurn != null:
return agentEnqueueTurn(_that.dest,_that.text,_that.inReplyTo);case UiCommandDto_AgentRespondPermission() when agentRespondPermission != null:
return agentRespondPermission(_that.dest,_that.callId,_that.permission);case UiCommandDto_AgentAbortTurn() when agentAbortTurn != null:
return agentAbortTurn(_that.dest);case UiCommandDto_AgentRunResult() when agentRunResult != null:
return agentRunResult(_that.dest,_that.profileId,_that.epoch,_that.output,_that.error);case UiCommandDto_SettingsPatch() when settingsPatch != null:
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
case UiCommandDto_SendText():
return sendText(_that.dest,_that.text,_that.kind);case UiCommandDto_SendMedia():
return sendMedia(_that.dest,_that.path,_that.mime,_that.width,_that.height,_that.byteSize,_that.kind);case UiCommandDto_RetrySend():
return retrySend(_that.clientId);case UiCommandDto_CancelSend():
return cancelSend(_that.clientId);case UiCommandDto_MarkThreadRead():
return markThreadRead(_that.dest,_that.kind,_that.visibleMessageId);case UiCommandDto_DeleteThread():
return deleteThread(_that.dest);case UiCommandDto_FriendRequest():
return friendRequest(_that.dest);case UiCommandDto_FriendAccept():
return friendAccept(_that.dest);case UiCommandDto_FriendReject():
return friendReject(_that.dest);case UiCommandDto_FriendRemove():
return friendRemove(_that.dest);case UiCommandDto_AgentEnqueueTurn():
return agentEnqueueTurn(_that.dest,_that.text,_that.inReplyTo);case UiCommandDto_AgentRespondPermission():
return agentRespondPermission(_that.dest,_that.callId,_that.permission);case UiCommandDto_AgentAbortTurn():
return agentAbortTurn(_that.dest);case UiCommandDto_AgentRunResult():
return agentRunResult(_that.dest,_that.profileId,_that.epoch,_that.output,_that.error);case UiCommandDto_SettingsPatch():
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
case UiCommandDto_SendText() when sendText != null:
return sendText(_that.dest,_that.text,_that.kind);case UiCommandDto_SendMedia() when sendMedia != null:
return sendMedia(_that.dest,_that.path,_that.mime,_that.width,_that.height,_that.byteSize,_that.kind);case UiCommandDto_RetrySend() when retrySend != null:
return retrySend(_that.clientId);case UiCommandDto_CancelSend() when cancelSend != null:
return cancelSend(_that.clientId);case UiCommandDto_MarkThreadRead() when markThreadRead != null:
return markThreadRead(_that.dest,_that.kind,_that.visibleMessageId);case UiCommandDto_DeleteThread() when deleteThread != null:
return deleteThread(_that.dest);case UiCommandDto_FriendRequest() when friendRequest != null:
return friendRequest(_that.dest);case UiCommandDto_FriendAccept() when friendAccept != null:
return friendAccept(_that.dest);case UiCommandDto_FriendReject() when friendReject != null:
return friendReject(_that.dest);case UiCommandDto_FriendRemove() when friendRemove != null:
return friendRemove(_that.dest);case UiCommandDto_AgentEnqueueTurn() when agentEnqueueTurn != null:
return agentEnqueueTurn(_that.dest,_that.text,_that.inReplyTo);case UiCommandDto_AgentRespondPermission() when agentRespondPermission != null:
return agentRespondPermission(_that.dest,_that.callId,_that.permission);case UiCommandDto_AgentAbortTurn() when agentAbortTurn != null:
return agentAbortTurn(_that.dest);case UiCommandDto_AgentRunResult() when agentRunResult != null:
return agentRunResult(_that.dest,_that.profileId,_that.epoch,_that.output,_that.error);case UiCommandDto_SettingsPatch() when settingsPatch != null:
return settingsPatch(_that.wsUrl,_that.httpOrigin,_that.env);case _:
  return null;

}
}

}

/// @nodoc


class UiCommandDto_SendText extends UiCommandDto {
  const UiCommandDto_SendText({required this.dest, required this.text, required this.kind}): super._();
  

 final  String dest;
 final  String text;
 final  int kind;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_SendTextCopyWith<UiCommandDto_SendText> get copyWith => _$UiCommandDto_SendTextCopyWithImpl<UiCommandDto_SendText>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_SendText&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.text, text) || other.text == text)&&(identical(other.kind, kind) || other.kind == kind));
}


@override
int get hashCode => Object.hash(runtimeType,dest,text,kind);

@override
String toString() {
  return 'UiCommandDto.sendText(dest: $dest, text: $text, kind: $kind)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_SendTextCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_SendTextCopyWith(UiCommandDto_SendText value, $Res Function(UiCommandDto_SendText) _then) = _$UiCommandDto_SendTextCopyWithImpl;
@useResult
$Res call({
 String dest, String text, int kind
});




}
/// @nodoc
class _$UiCommandDto_SendTextCopyWithImpl<$Res>
    implements $UiCommandDto_SendTextCopyWith<$Res> {
  _$UiCommandDto_SendTextCopyWithImpl(this._self, this._then);

  final UiCommandDto_SendText _self;
  final $Res Function(UiCommandDto_SendText) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? text = null,Object? kind = null,}) {
  return _then(UiCommandDto_SendText(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class UiCommandDto_SendMedia extends UiCommandDto {
  const UiCommandDto_SendMedia({required this.dest, required this.path, required this.mime, required this.width, required this.height, required this.byteSize, required this.kind}): super._();
  

 final  String dest;
 final  String path;
 final  String mime;
 final  int width;
 final  int height;
 final  PlatformInt64 byteSize;
 final  int kind;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_SendMediaCopyWith<UiCommandDto_SendMedia> get copyWith => _$UiCommandDto_SendMediaCopyWithImpl<UiCommandDto_SendMedia>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_SendMedia&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.path, path) || other.path == path)&&(identical(other.mime, mime) || other.mime == mime)&&(identical(other.width, width) || other.width == width)&&(identical(other.height, height) || other.height == height)&&(identical(other.byteSize, byteSize) || other.byteSize == byteSize)&&(identical(other.kind, kind) || other.kind == kind));
}


@override
int get hashCode => Object.hash(runtimeType,dest,path,mime,width,height,byteSize,kind);

@override
String toString() {
  return 'UiCommandDto.sendMedia(dest: $dest, path: $path, mime: $mime, width: $width, height: $height, byteSize: $byteSize, kind: $kind)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_SendMediaCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_SendMediaCopyWith(UiCommandDto_SendMedia value, $Res Function(UiCommandDto_SendMedia) _then) = _$UiCommandDto_SendMediaCopyWithImpl;
@useResult
$Res call({
 String dest, String path, String mime, int width, int height, PlatformInt64 byteSize, int kind
});




}
/// @nodoc
class _$UiCommandDto_SendMediaCopyWithImpl<$Res>
    implements $UiCommandDto_SendMediaCopyWith<$Res> {
  _$UiCommandDto_SendMediaCopyWithImpl(this._self, this._then);

  final UiCommandDto_SendMedia _self;
  final $Res Function(UiCommandDto_SendMedia) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? path = null,Object? mime = null,Object? width = null,Object? height = null,Object? byteSize = null,Object? kind = null,}) {
  return _then(UiCommandDto_SendMedia(
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


class UiCommandDto_RetrySend extends UiCommandDto {
  const UiCommandDto_RetrySend({required this.clientId}): super._();
  

 final  String clientId;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_RetrySendCopyWith<UiCommandDto_RetrySend> get copyWith => _$UiCommandDto_RetrySendCopyWithImpl<UiCommandDto_RetrySend>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_RetrySend&&(identical(other.clientId, clientId) || other.clientId == clientId));
}


@override
int get hashCode => Object.hash(runtimeType,clientId);

@override
String toString() {
  return 'UiCommandDto.retrySend(clientId: $clientId)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_RetrySendCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_RetrySendCopyWith(UiCommandDto_RetrySend value, $Res Function(UiCommandDto_RetrySend) _then) = _$UiCommandDto_RetrySendCopyWithImpl;
@useResult
$Res call({
 String clientId
});




}
/// @nodoc
class _$UiCommandDto_RetrySendCopyWithImpl<$Res>
    implements $UiCommandDto_RetrySendCopyWith<$Res> {
  _$UiCommandDto_RetrySendCopyWithImpl(this._self, this._then);

  final UiCommandDto_RetrySend _self;
  final $Res Function(UiCommandDto_RetrySend) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? clientId = null,}) {
  return _then(UiCommandDto_RetrySend(
clientId: null == clientId ? _self.clientId : clientId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_CancelSend extends UiCommandDto {
  const UiCommandDto_CancelSend({required this.clientId}): super._();
  

 final  String clientId;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_CancelSendCopyWith<UiCommandDto_CancelSend> get copyWith => _$UiCommandDto_CancelSendCopyWithImpl<UiCommandDto_CancelSend>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_CancelSend&&(identical(other.clientId, clientId) || other.clientId == clientId));
}


@override
int get hashCode => Object.hash(runtimeType,clientId);

@override
String toString() {
  return 'UiCommandDto.cancelSend(clientId: $clientId)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_CancelSendCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_CancelSendCopyWith(UiCommandDto_CancelSend value, $Res Function(UiCommandDto_CancelSend) _then) = _$UiCommandDto_CancelSendCopyWithImpl;
@useResult
$Res call({
 String clientId
});




}
/// @nodoc
class _$UiCommandDto_CancelSendCopyWithImpl<$Res>
    implements $UiCommandDto_CancelSendCopyWith<$Res> {
  _$UiCommandDto_CancelSendCopyWithImpl(this._self, this._then);

  final UiCommandDto_CancelSend _self;
  final $Res Function(UiCommandDto_CancelSend) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? clientId = null,}) {
  return _then(UiCommandDto_CancelSend(
clientId: null == clientId ? _self.clientId : clientId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_MarkThreadRead extends UiCommandDto {
  const UiCommandDto_MarkThreadRead({required this.dest, required this.kind, required this.visibleMessageId}): super._();
  

 final  String dest;
 final  int kind;
 final  PlatformInt64 visibleMessageId;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_MarkThreadReadCopyWith<UiCommandDto_MarkThreadRead> get copyWith => _$UiCommandDto_MarkThreadReadCopyWithImpl<UiCommandDto_MarkThreadRead>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_MarkThreadRead&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.visibleMessageId, visibleMessageId) || other.visibleMessageId == visibleMessageId));
}


@override
int get hashCode => Object.hash(runtimeType,dest,kind,visibleMessageId);

@override
String toString() {
  return 'UiCommandDto.markThreadRead(dest: $dest, kind: $kind, visibleMessageId: $visibleMessageId)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_MarkThreadReadCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_MarkThreadReadCopyWith(UiCommandDto_MarkThreadRead value, $Res Function(UiCommandDto_MarkThreadRead) _then) = _$UiCommandDto_MarkThreadReadCopyWithImpl;
@useResult
$Res call({
 String dest, int kind, PlatformInt64 visibleMessageId
});




}
/// @nodoc
class _$UiCommandDto_MarkThreadReadCopyWithImpl<$Res>
    implements $UiCommandDto_MarkThreadReadCopyWith<$Res> {
  _$UiCommandDto_MarkThreadReadCopyWithImpl(this._self, this._then);

  final UiCommandDto_MarkThreadRead _self;
  final $Res Function(UiCommandDto_MarkThreadRead) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? kind = null,Object? visibleMessageId = null,}) {
  return _then(UiCommandDto_MarkThreadRead(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as int,visibleMessageId: null == visibleMessageId ? _self.visibleMessageId : visibleMessageId // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class UiCommandDto_DeleteThread extends UiCommandDto {
  const UiCommandDto_DeleteThread({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_DeleteThreadCopyWith<UiCommandDto_DeleteThread> get copyWith => _$UiCommandDto_DeleteThreadCopyWithImpl<UiCommandDto_DeleteThread>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_DeleteThread&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommandDto.deleteThread(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_DeleteThreadCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_DeleteThreadCopyWith(UiCommandDto_DeleteThread value, $Res Function(UiCommandDto_DeleteThread) _then) = _$UiCommandDto_DeleteThreadCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommandDto_DeleteThreadCopyWithImpl<$Res>
    implements $UiCommandDto_DeleteThreadCopyWith<$Res> {
  _$UiCommandDto_DeleteThreadCopyWithImpl(this._self, this._then);

  final UiCommandDto_DeleteThread _self;
  final $Res Function(UiCommandDto_DeleteThread) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommandDto_DeleteThread(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_FriendRequest extends UiCommandDto {
  const UiCommandDto_FriendRequest({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_FriendRequestCopyWith<UiCommandDto_FriendRequest> get copyWith => _$UiCommandDto_FriendRequestCopyWithImpl<UiCommandDto_FriendRequest>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_FriendRequest&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommandDto.friendRequest(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_FriendRequestCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_FriendRequestCopyWith(UiCommandDto_FriendRequest value, $Res Function(UiCommandDto_FriendRequest) _then) = _$UiCommandDto_FriendRequestCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommandDto_FriendRequestCopyWithImpl<$Res>
    implements $UiCommandDto_FriendRequestCopyWith<$Res> {
  _$UiCommandDto_FriendRequestCopyWithImpl(this._self, this._then);

  final UiCommandDto_FriendRequest _self;
  final $Res Function(UiCommandDto_FriendRequest) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommandDto_FriendRequest(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_FriendAccept extends UiCommandDto {
  const UiCommandDto_FriendAccept({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_FriendAcceptCopyWith<UiCommandDto_FriendAccept> get copyWith => _$UiCommandDto_FriendAcceptCopyWithImpl<UiCommandDto_FriendAccept>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_FriendAccept&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommandDto.friendAccept(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_FriendAcceptCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_FriendAcceptCopyWith(UiCommandDto_FriendAccept value, $Res Function(UiCommandDto_FriendAccept) _then) = _$UiCommandDto_FriendAcceptCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommandDto_FriendAcceptCopyWithImpl<$Res>
    implements $UiCommandDto_FriendAcceptCopyWith<$Res> {
  _$UiCommandDto_FriendAcceptCopyWithImpl(this._self, this._then);

  final UiCommandDto_FriendAccept _self;
  final $Res Function(UiCommandDto_FriendAccept) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommandDto_FriendAccept(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_FriendReject extends UiCommandDto {
  const UiCommandDto_FriendReject({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_FriendRejectCopyWith<UiCommandDto_FriendReject> get copyWith => _$UiCommandDto_FriendRejectCopyWithImpl<UiCommandDto_FriendReject>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_FriendReject&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommandDto.friendReject(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_FriendRejectCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_FriendRejectCopyWith(UiCommandDto_FriendReject value, $Res Function(UiCommandDto_FriendReject) _then) = _$UiCommandDto_FriendRejectCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommandDto_FriendRejectCopyWithImpl<$Res>
    implements $UiCommandDto_FriendRejectCopyWith<$Res> {
  _$UiCommandDto_FriendRejectCopyWithImpl(this._self, this._then);

  final UiCommandDto_FriendReject _self;
  final $Res Function(UiCommandDto_FriendReject) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommandDto_FriendReject(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_FriendRemove extends UiCommandDto {
  const UiCommandDto_FriendRemove({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_FriendRemoveCopyWith<UiCommandDto_FriendRemove> get copyWith => _$UiCommandDto_FriendRemoveCopyWithImpl<UiCommandDto_FriendRemove>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_FriendRemove&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommandDto.friendRemove(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_FriendRemoveCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_FriendRemoveCopyWith(UiCommandDto_FriendRemove value, $Res Function(UiCommandDto_FriendRemove) _then) = _$UiCommandDto_FriendRemoveCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommandDto_FriendRemoveCopyWithImpl<$Res>
    implements $UiCommandDto_FriendRemoveCopyWith<$Res> {
  _$UiCommandDto_FriendRemoveCopyWithImpl(this._self, this._then);

  final UiCommandDto_FriendRemove _self;
  final $Res Function(UiCommandDto_FriendRemove) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommandDto_FriendRemove(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_AgentEnqueueTurn extends UiCommandDto {
  const UiCommandDto_AgentEnqueueTurn({required this.dest, required this.text, required this.inReplyTo}): super._();
  

 final  String dest;
 final  String text;
 final  PlatformInt64 inReplyTo;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_AgentEnqueueTurnCopyWith<UiCommandDto_AgentEnqueueTurn> get copyWith => _$UiCommandDto_AgentEnqueueTurnCopyWithImpl<UiCommandDto_AgentEnqueueTurn>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_AgentEnqueueTurn&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.text, text) || other.text == text)&&(identical(other.inReplyTo, inReplyTo) || other.inReplyTo == inReplyTo));
}


@override
int get hashCode => Object.hash(runtimeType,dest,text,inReplyTo);

@override
String toString() {
  return 'UiCommandDto.agentEnqueueTurn(dest: $dest, text: $text, inReplyTo: $inReplyTo)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_AgentEnqueueTurnCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_AgentEnqueueTurnCopyWith(UiCommandDto_AgentEnqueueTurn value, $Res Function(UiCommandDto_AgentEnqueueTurn) _then) = _$UiCommandDto_AgentEnqueueTurnCopyWithImpl;
@useResult
$Res call({
 String dest, String text, PlatformInt64 inReplyTo
});




}
/// @nodoc
class _$UiCommandDto_AgentEnqueueTurnCopyWithImpl<$Res>
    implements $UiCommandDto_AgentEnqueueTurnCopyWith<$Res> {
  _$UiCommandDto_AgentEnqueueTurnCopyWithImpl(this._self, this._then);

  final UiCommandDto_AgentEnqueueTurn _self;
  final $Res Function(UiCommandDto_AgentEnqueueTurn) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? text = null,Object? inReplyTo = null,}) {
  return _then(UiCommandDto_AgentEnqueueTurn(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,inReplyTo: null == inReplyTo ? _self.inReplyTo : inReplyTo // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class UiCommandDto_AgentRespondPermission extends UiCommandDto {
  const UiCommandDto_AgentRespondPermission({required this.dest, required this.callId, required this.permission}): super._();
  

 final  String dest;
 final  String callId;
 final  String permission;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_AgentRespondPermissionCopyWith<UiCommandDto_AgentRespondPermission> get copyWith => _$UiCommandDto_AgentRespondPermissionCopyWithImpl<UiCommandDto_AgentRespondPermission>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_AgentRespondPermission&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.callId, callId) || other.callId == callId)&&(identical(other.permission, permission) || other.permission == permission));
}


@override
int get hashCode => Object.hash(runtimeType,dest,callId,permission);

@override
String toString() {
  return 'UiCommandDto.agentRespondPermission(dest: $dest, callId: $callId, permission: $permission)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_AgentRespondPermissionCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_AgentRespondPermissionCopyWith(UiCommandDto_AgentRespondPermission value, $Res Function(UiCommandDto_AgentRespondPermission) _then) = _$UiCommandDto_AgentRespondPermissionCopyWithImpl;
@useResult
$Res call({
 String dest, String callId, String permission
});




}
/// @nodoc
class _$UiCommandDto_AgentRespondPermissionCopyWithImpl<$Res>
    implements $UiCommandDto_AgentRespondPermissionCopyWith<$Res> {
  _$UiCommandDto_AgentRespondPermissionCopyWithImpl(this._self, this._then);

  final UiCommandDto_AgentRespondPermission _self;
  final $Res Function(UiCommandDto_AgentRespondPermission) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? callId = null,Object? permission = null,}) {
  return _then(UiCommandDto_AgentRespondPermission(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,permission: null == permission ? _self.permission : permission // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_AgentAbortTurn extends UiCommandDto {
  const UiCommandDto_AgentAbortTurn({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_AgentAbortTurnCopyWith<UiCommandDto_AgentAbortTurn> get copyWith => _$UiCommandDto_AgentAbortTurnCopyWithImpl<UiCommandDto_AgentAbortTurn>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_AgentAbortTurn&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'UiCommandDto.agentAbortTurn(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_AgentAbortTurnCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_AgentAbortTurnCopyWith(UiCommandDto_AgentAbortTurn value, $Res Function(UiCommandDto_AgentAbortTurn) _then) = _$UiCommandDto_AgentAbortTurnCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$UiCommandDto_AgentAbortTurnCopyWithImpl<$Res>
    implements $UiCommandDto_AgentAbortTurnCopyWith<$Res> {
  _$UiCommandDto_AgentAbortTurnCopyWithImpl(this._self, this._then);

  final UiCommandDto_AgentAbortTurn _self;
  final $Res Function(UiCommandDto_AgentAbortTurn) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(UiCommandDto_AgentAbortTurn(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCommandDto_AgentRunResult extends UiCommandDto {
  const UiCommandDto_AgentRunResult({required this.dest, required this.profileId, required this.epoch, required this.output, this.error}): super._();
  

 final  String dest;
 final  String profileId;
 final  BigInt epoch;
 final  String output;
 final  String? error;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_AgentRunResultCopyWith<UiCommandDto_AgentRunResult> get copyWith => _$UiCommandDto_AgentRunResultCopyWithImpl<UiCommandDto_AgentRunResult>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_AgentRunResult&&(identical(other.dest, dest) || other.dest == dest)&&(identical(other.profileId, profileId) || other.profileId == profileId)&&(identical(other.epoch, epoch) || other.epoch == epoch)&&(identical(other.output, output) || other.output == output)&&(identical(other.error, error) || other.error == error));
}


@override
int get hashCode => Object.hash(runtimeType,dest,profileId,epoch,output,error);

@override
String toString() {
  return 'UiCommandDto.agentRunResult(dest: $dest, profileId: $profileId, epoch: $epoch, output: $output, error: $error)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_AgentRunResultCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_AgentRunResultCopyWith(UiCommandDto_AgentRunResult value, $Res Function(UiCommandDto_AgentRunResult) _then) = _$UiCommandDto_AgentRunResultCopyWithImpl;
@useResult
$Res call({
 String dest, String profileId, BigInt epoch, String output, String? error
});




}
/// @nodoc
class _$UiCommandDto_AgentRunResultCopyWithImpl<$Res>
    implements $UiCommandDto_AgentRunResultCopyWith<$Res> {
  _$UiCommandDto_AgentRunResultCopyWithImpl(this._self, this._then);

  final UiCommandDto_AgentRunResult _self;
  final $Res Function(UiCommandDto_AgentRunResult) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,Object? profileId = null,Object? epoch = null,Object? output = null,Object? error = freezed,}) {
  return _then(UiCommandDto_AgentRunResult(
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


class UiCommandDto_SettingsPatch extends UiCommandDto {
  const UiCommandDto_SettingsPatch({this.wsUrl, this.httpOrigin, this.env}): super._();
  

 final  String? wsUrl;
 final  String? httpOrigin;
 final  String? env;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCommandDto_SettingsPatchCopyWith<UiCommandDto_SettingsPatch> get copyWith => _$UiCommandDto_SettingsPatchCopyWithImpl<UiCommandDto_SettingsPatch>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCommandDto_SettingsPatch&&(identical(other.wsUrl, wsUrl) || other.wsUrl == wsUrl)&&(identical(other.httpOrigin, httpOrigin) || other.httpOrigin == httpOrigin)&&(identical(other.env, env) || other.env == env));
}


@override
int get hashCode => Object.hash(runtimeType,wsUrl,httpOrigin,env);

@override
String toString() {
  return 'UiCommandDto.settingsPatch(wsUrl: $wsUrl, httpOrigin: $httpOrigin, env: $env)';
}


}

/// @nodoc
abstract mixin class $UiCommandDto_SettingsPatchCopyWith<$Res> implements $UiCommandDtoCopyWith<$Res> {
  factory $UiCommandDto_SettingsPatchCopyWith(UiCommandDto_SettingsPatch value, $Res Function(UiCommandDto_SettingsPatch) _then) = _$UiCommandDto_SettingsPatchCopyWithImpl;
@useResult
$Res call({
 String? wsUrl, String? httpOrigin, String? env
});




}
/// @nodoc
class _$UiCommandDto_SettingsPatchCopyWithImpl<$Res>
    implements $UiCommandDto_SettingsPatchCopyWith<$Res> {
  _$UiCommandDto_SettingsPatchCopyWithImpl(this._self, this._then);

  final UiCommandDto_SettingsPatch _self;
  final $Res Function(UiCommandDto_SettingsPatch) _then;

/// Create a copy of UiCommandDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? wsUrl = freezed,Object? httpOrigin = freezed,Object? env = freezed,}) {
  return _then(UiCommandDto_SettingsPatch(
wsUrl: freezed == wsUrl ? _self.wsUrl : wsUrl // ignore: cast_nullable_to_non_nullable
as String?,httpOrigin: freezed == httpOrigin ? _self.httpOrigin : httpOrigin // ignore: cast_nullable_to_non_nullable
as String?,env: freezed == env ? _self.env : env // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

// dart format on
