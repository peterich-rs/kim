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
mixin _$OutgoingContent {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutgoingContent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutgoingContent()';
}


}

/// @nodoc
class $OutgoingContentCopyWith<$Res>  {
$OutgoingContentCopyWith(OutgoingContent _, $Res Function(OutgoingContent) __);
}


/// Adds pattern-matching-related methods to [OutgoingContent].
extension OutgoingContentPatterns on OutgoingContent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( OutgoingContent_Text value)?  text,TResult Function( OutgoingContent_Image value)?  image,TResult Function( OutgoingContent_Video value)?  video,TResult Function( OutgoingContent_Voice value)?  voice,required TResult orElse(),}){
final _that = this;
switch (_that) {
case OutgoingContent_Text() when text != null:
return text(_that);case OutgoingContent_Image() when image != null:
return image(_that);case OutgoingContent_Video() when video != null:
return video(_that);case OutgoingContent_Voice() when voice != null:
return voice(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( OutgoingContent_Text value)  text,required TResult Function( OutgoingContent_Image value)  image,required TResult Function( OutgoingContent_Video value)  video,required TResult Function( OutgoingContent_Voice value)  voice,}){
final _that = this;
switch (_that) {
case OutgoingContent_Text():
return text(_that);case OutgoingContent_Image():
return image(_that);case OutgoingContent_Video():
return video(_that);case OutgoingContent_Voice():
return voice(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( OutgoingContent_Text value)?  text,TResult? Function( OutgoingContent_Image value)?  image,TResult? Function( OutgoingContent_Video value)?  video,TResult? Function( OutgoingContent_Voice value)?  voice,}){
final _that = this;
switch (_that) {
case OutgoingContent_Text() when text != null:
return text(_that);case OutgoingContent_Image() when image != null:
return image(_that);case OutgoingContent_Video() when video != null:
return video(_that);case OutgoingContent_Voice() when voice != null:
return voice(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String body)?  text,TResult Function( String path,  String mime,  int width,  int height,  PlatformInt64 byteSize)?  image,TResult Function( String path,  PlatformInt64 byteSize)?  video,TResult Function( String path,  PlatformInt64 byteSize)?  voice,required TResult orElse(),}) {final _that = this;
switch (_that) {
case OutgoingContent_Text() when text != null:
return text(_that.body);case OutgoingContent_Image() when image != null:
return image(_that.path,_that.mime,_that.width,_that.height,_that.byteSize);case OutgoingContent_Video() when video != null:
return video(_that.path,_that.byteSize);case OutgoingContent_Voice() when voice != null:
return voice(_that.path,_that.byteSize);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String body)  text,required TResult Function( String path,  String mime,  int width,  int height,  PlatformInt64 byteSize)  image,required TResult Function( String path,  PlatformInt64 byteSize)  video,required TResult Function( String path,  PlatformInt64 byteSize)  voice,}) {final _that = this;
switch (_that) {
case OutgoingContent_Text():
return text(_that.body);case OutgoingContent_Image():
return image(_that.path,_that.mime,_that.width,_that.height,_that.byteSize);case OutgoingContent_Video():
return video(_that.path,_that.byteSize);case OutgoingContent_Voice():
return voice(_that.path,_that.byteSize);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String body)?  text,TResult? Function( String path,  String mime,  int width,  int height,  PlatformInt64 byteSize)?  image,TResult? Function( String path,  PlatformInt64 byteSize)?  video,TResult? Function( String path,  PlatformInt64 byteSize)?  voice,}) {final _that = this;
switch (_that) {
case OutgoingContent_Text() when text != null:
return text(_that.body);case OutgoingContent_Image() when image != null:
return image(_that.path,_that.mime,_that.width,_that.height,_that.byteSize);case OutgoingContent_Video() when video != null:
return video(_that.path,_that.byteSize);case OutgoingContent_Voice() when voice != null:
return voice(_that.path,_that.byteSize);case _:
  return null;

}
}

}

/// @nodoc


class OutgoingContent_Text extends OutgoingContent {
  const OutgoingContent_Text({required this.body}): super._();
  

 final  String body;

/// Create a copy of OutgoingContent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$OutgoingContent_TextCopyWith<OutgoingContent_Text> get copyWith => _$OutgoingContent_TextCopyWithImpl<OutgoingContent_Text>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutgoingContent_Text&&(identical(other.body, body) || other.body == body));
}


@override
int get hashCode => Object.hash(runtimeType,body);

@override
String toString() {
  return 'OutgoingContent.text(body: $body)';
}


}

/// @nodoc
abstract mixin class $OutgoingContent_TextCopyWith<$Res> implements $OutgoingContentCopyWith<$Res> {
  factory $OutgoingContent_TextCopyWith(OutgoingContent_Text value, $Res Function(OutgoingContent_Text) _then) = _$OutgoingContent_TextCopyWithImpl;
@useResult
$Res call({
 String body
});




}
/// @nodoc
class _$OutgoingContent_TextCopyWithImpl<$Res>
    implements $OutgoingContent_TextCopyWith<$Res> {
  _$OutgoingContent_TextCopyWithImpl(this._self, this._then);

  final OutgoingContent_Text _self;
  final $Res Function(OutgoingContent_Text) _then;

/// Create a copy of OutgoingContent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? body = null,}) {
  return _then(OutgoingContent_Text(
body: null == body ? _self.body : body // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class OutgoingContent_Image extends OutgoingContent {
  const OutgoingContent_Image({required this.path, required this.mime, required this.width, required this.height, required this.byteSize}): super._();
  

 final  String path;
 final  String mime;
 final  int width;
 final  int height;
 final  PlatformInt64 byteSize;

/// Create a copy of OutgoingContent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$OutgoingContent_ImageCopyWith<OutgoingContent_Image> get copyWith => _$OutgoingContent_ImageCopyWithImpl<OutgoingContent_Image>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutgoingContent_Image&&(identical(other.path, path) || other.path == path)&&(identical(other.mime, mime) || other.mime == mime)&&(identical(other.width, width) || other.width == width)&&(identical(other.height, height) || other.height == height)&&(identical(other.byteSize, byteSize) || other.byteSize == byteSize));
}


@override
int get hashCode => Object.hash(runtimeType,path,mime,width,height,byteSize);

@override
String toString() {
  return 'OutgoingContent.image(path: $path, mime: $mime, width: $width, height: $height, byteSize: $byteSize)';
}


}

/// @nodoc
abstract mixin class $OutgoingContent_ImageCopyWith<$Res> implements $OutgoingContentCopyWith<$Res> {
  factory $OutgoingContent_ImageCopyWith(OutgoingContent_Image value, $Res Function(OutgoingContent_Image) _then) = _$OutgoingContent_ImageCopyWithImpl;
@useResult
$Res call({
 String path, String mime, int width, int height, PlatformInt64 byteSize
});




}
/// @nodoc
class _$OutgoingContent_ImageCopyWithImpl<$Res>
    implements $OutgoingContent_ImageCopyWith<$Res> {
  _$OutgoingContent_ImageCopyWithImpl(this._self, this._then);

  final OutgoingContent_Image _self;
  final $Res Function(OutgoingContent_Image) _then;

/// Create a copy of OutgoingContent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,Object? mime = null,Object? width = null,Object? height = null,Object? byteSize = null,}) {
  return _then(OutgoingContent_Image(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,mime: null == mime ? _self.mime : mime // ignore: cast_nullable_to_non_nullable
as String,width: null == width ? _self.width : width // ignore: cast_nullable_to_non_nullable
as int,height: null == height ? _self.height : height // ignore: cast_nullable_to_non_nullable
as int,byteSize: null == byteSize ? _self.byteSize : byteSize // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class OutgoingContent_Video extends OutgoingContent {
  const OutgoingContent_Video({required this.path, required this.byteSize}): super._();
  

 final  String path;
 final  PlatformInt64 byteSize;

/// Create a copy of OutgoingContent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$OutgoingContent_VideoCopyWith<OutgoingContent_Video> get copyWith => _$OutgoingContent_VideoCopyWithImpl<OutgoingContent_Video>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutgoingContent_Video&&(identical(other.path, path) || other.path == path)&&(identical(other.byteSize, byteSize) || other.byteSize == byteSize));
}


@override
int get hashCode => Object.hash(runtimeType,path,byteSize);

@override
String toString() {
  return 'OutgoingContent.video(path: $path, byteSize: $byteSize)';
}


}

/// @nodoc
abstract mixin class $OutgoingContent_VideoCopyWith<$Res> implements $OutgoingContentCopyWith<$Res> {
  factory $OutgoingContent_VideoCopyWith(OutgoingContent_Video value, $Res Function(OutgoingContent_Video) _then) = _$OutgoingContent_VideoCopyWithImpl;
@useResult
$Res call({
 String path, PlatformInt64 byteSize
});




}
/// @nodoc
class _$OutgoingContent_VideoCopyWithImpl<$Res>
    implements $OutgoingContent_VideoCopyWith<$Res> {
  _$OutgoingContent_VideoCopyWithImpl(this._self, this._then);

  final OutgoingContent_Video _self;
  final $Res Function(OutgoingContent_Video) _then;

/// Create a copy of OutgoingContent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,Object? byteSize = null,}) {
  return _then(OutgoingContent_Video(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,byteSize: null == byteSize ? _self.byteSize : byteSize // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class OutgoingContent_Voice extends OutgoingContent {
  const OutgoingContent_Voice({required this.path, required this.byteSize}): super._();
  

 final  String path;
 final  PlatformInt64 byteSize;

/// Create a copy of OutgoingContent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$OutgoingContent_VoiceCopyWith<OutgoingContent_Voice> get copyWith => _$OutgoingContent_VoiceCopyWithImpl<OutgoingContent_Voice>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutgoingContent_Voice&&(identical(other.path, path) || other.path == path)&&(identical(other.byteSize, byteSize) || other.byteSize == byteSize));
}


@override
int get hashCode => Object.hash(runtimeType,path,byteSize);

@override
String toString() {
  return 'OutgoingContent.voice(path: $path, byteSize: $byteSize)';
}


}

/// @nodoc
abstract mixin class $OutgoingContent_VoiceCopyWith<$Res> implements $OutgoingContentCopyWith<$Res> {
  factory $OutgoingContent_VoiceCopyWith(OutgoingContent_Voice value, $Res Function(OutgoingContent_Voice) _then) = _$OutgoingContent_VoiceCopyWithImpl;
@useResult
$Res call({
 String path, PlatformInt64 byteSize
});




}
/// @nodoc
class _$OutgoingContent_VoiceCopyWithImpl<$Res>
    implements $OutgoingContent_VoiceCopyWith<$Res> {
  _$OutgoingContent_VoiceCopyWithImpl(this._self, this._then);

  final OutgoingContent_Voice _self;
  final $Res Function(OutgoingContent_Voice) _then;

/// Create a copy of OutgoingContent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,Object? byteSize = null,}) {
  return _then(OutgoingContent_Voice(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,byteSize: null == byteSize ? _self.byteSize : byteSize // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SessionUpdate_Link value)?  link,TResult Function( SessionUpdate_Inbox value)?  inbox,TResult Function( SessionUpdate_ThreadUpsert value)?  threadUpsert,TResult Function( SessionUpdate_SyncProgress value)?  syncProgress,TResult Function( SessionUpdate_Kickout value)?  kickout,TResult Function( SessionUpdate_AuthExpired value)?  authExpired,TResult Function( SessionUpdate_FriendRequest value)?  friendRequest,TResult Function( SessionUpdate_FriendAccepted value)?  friendAccepted,TResult Function( SessionUpdate_ProfileUpdated value)?  profileUpdated,TResult Function( SessionUpdate_Presence value)?  presence,TResult Function( SessionUpdate_Typing value)?  typing,TResult Function( SessionUpdate_ReceiptRead value)?  receiptRead,TResult Function( SessionUpdate_GroupCreate value)?  groupCreate,TResult Function( SessionUpdate_ContactsChanged value)?  contactsChanged,TResult Function( SessionUpdate_AgentTurn value)?  agentTurn,TResult Function( SessionUpdate_AgentCard value)?  agentCard,TResult Function( SessionUpdate_RustPanic value)?  rustPanic,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SessionUpdate_Link() when link != null:
return link(_that);case SessionUpdate_Inbox() when inbox != null:
return inbox(_that);case SessionUpdate_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that);case SessionUpdate_SyncProgress() when syncProgress != null:
return syncProgress(_that);case SessionUpdate_Kickout() when kickout != null:
return kickout(_that);case SessionUpdate_AuthExpired() when authExpired != null:
return authExpired(_that);case SessionUpdate_FriendRequest() when friendRequest != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SessionUpdate_Link value)  link,required TResult Function( SessionUpdate_Inbox value)  inbox,required TResult Function( SessionUpdate_ThreadUpsert value)  threadUpsert,required TResult Function( SessionUpdate_SyncProgress value)  syncProgress,required TResult Function( SessionUpdate_Kickout value)  kickout,required TResult Function( SessionUpdate_AuthExpired value)  authExpired,required TResult Function( SessionUpdate_FriendRequest value)  friendRequest,required TResult Function( SessionUpdate_FriendAccepted value)  friendAccepted,required TResult Function( SessionUpdate_ProfileUpdated value)  profileUpdated,required TResult Function( SessionUpdate_Presence value)  presence,required TResult Function( SessionUpdate_Typing value)  typing,required TResult Function( SessionUpdate_ReceiptRead value)  receiptRead,required TResult Function( SessionUpdate_GroupCreate value)  groupCreate,required TResult Function( SessionUpdate_ContactsChanged value)  contactsChanged,required TResult Function( SessionUpdate_AgentTurn value)  agentTurn,required TResult Function( SessionUpdate_AgentCard value)  agentCard,required TResult Function( SessionUpdate_RustPanic value)  rustPanic,}){
final _that = this;
switch (_that) {
case SessionUpdate_Link():
return link(_that);case SessionUpdate_Inbox():
return inbox(_that);case SessionUpdate_ThreadUpsert():
return threadUpsert(_that);case SessionUpdate_SyncProgress():
return syncProgress(_that);case SessionUpdate_Kickout():
return kickout(_that);case SessionUpdate_AuthExpired():
return authExpired(_that);case SessionUpdate_FriendRequest():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SessionUpdate_Link value)?  link,TResult? Function( SessionUpdate_Inbox value)?  inbox,TResult? Function( SessionUpdate_ThreadUpsert value)?  threadUpsert,TResult? Function( SessionUpdate_SyncProgress value)?  syncProgress,TResult? Function( SessionUpdate_Kickout value)?  kickout,TResult? Function( SessionUpdate_AuthExpired value)?  authExpired,TResult? Function( SessionUpdate_FriendRequest value)?  friendRequest,TResult? Function( SessionUpdate_FriendAccepted value)?  friendAccepted,TResult? Function( SessionUpdate_ProfileUpdated value)?  profileUpdated,TResult? Function( SessionUpdate_Presence value)?  presence,TResult? Function( SessionUpdate_Typing value)?  typing,TResult? Function( SessionUpdate_ReceiptRead value)?  receiptRead,TResult? Function( SessionUpdate_GroupCreate value)?  groupCreate,TResult? Function( SessionUpdate_ContactsChanged value)?  contactsChanged,TResult? Function( SessionUpdate_AgentTurn value)?  agentTurn,TResult? Function( SessionUpdate_AgentCard value)?  agentCard,TResult? Function( SessionUpdate_RustPanic value)?  rustPanic,}){
final _that = this;
switch (_that) {
case SessionUpdate_Link() when link != null:
return link(_that);case SessionUpdate_Inbox() when inbox != null:
return inbox(_that);case SessionUpdate_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that);case SessionUpdate_SyncProgress() when syncProgress != null:
return syncProgress(_that);case SessionUpdate_Kickout() when kickout != null:
return kickout(_that);case SessionUpdate_AuthExpired() when authExpired != null:
return authExpired(_that);case SessionUpdate_FriendRequest() when friendRequest != null:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( LinkState state,  String? lastError)?  link,TResult Function( List<ThreadView> threads)?  inbox,TResult Function( ThreadView thread)?  threadUpsert,TResult Function( BigInt pulled,  bool catchingUp)?  syncProgress,TResult Function( String channelId)?  kickout,TResult Function( String reason)?  authExpired,TResult Function( String from,  String nickname)?  friendRequest,TResult Function( String from,  String nickname)?  friendAccepted,TResult Function( String account,  String nickname,  String avatar)?  profileUpdated,TResult Function( String account,  int status,  PlatformInt64 lastSeen)?  presence,TResult Function( String typer,  String dest,  ThreadKind kind,  bool active)?  typing,TResult Function( String reader,  String dest,  ThreadKind kind,  PlatformInt64 messageId)?  receiptRead,TResult Function( String groupId,  List<String> members)?  groupCreate,TResult Function( List<Person> contacts)?  contactsChanged,TResult Function( String dest,  AgentTurnState state,  String text)?  agentTurn,TResult Function( String dest,  AgentCard card)?  agentCard,TResult Function( String message)?  rustPanic,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SessionUpdate_Link() when link != null:
return link(_that.state,_that.lastError);case SessionUpdate_Inbox() when inbox != null:
return inbox(_that.threads);case SessionUpdate_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that.thread);case SessionUpdate_SyncProgress() when syncProgress != null:
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdate_Kickout() when kickout != null:
return kickout(_that.channelId);case SessionUpdate_AuthExpired() when authExpired != null:
return authExpired(_that.reason);case SessionUpdate_FriendRequest() when friendRequest != null:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( LinkState state,  String? lastError)  link,required TResult Function( List<ThreadView> threads)  inbox,required TResult Function( ThreadView thread)  threadUpsert,required TResult Function( BigInt pulled,  bool catchingUp)  syncProgress,required TResult Function( String channelId)  kickout,required TResult Function( String reason)  authExpired,required TResult Function( String from,  String nickname)  friendRequest,required TResult Function( String from,  String nickname)  friendAccepted,required TResult Function( String account,  String nickname,  String avatar)  profileUpdated,required TResult Function( String account,  int status,  PlatformInt64 lastSeen)  presence,required TResult Function( String typer,  String dest,  ThreadKind kind,  bool active)  typing,required TResult Function( String reader,  String dest,  ThreadKind kind,  PlatformInt64 messageId)  receiptRead,required TResult Function( String groupId,  List<String> members)  groupCreate,required TResult Function( List<Person> contacts)  contactsChanged,required TResult Function( String dest,  AgentTurnState state,  String text)  agentTurn,required TResult Function( String dest,  AgentCard card)  agentCard,required TResult Function( String message)  rustPanic,}) {final _that = this;
switch (_that) {
case SessionUpdate_Link():
return link(_that.state,_that.lastError);case SessionUpdate_Inbox():
return inbox(_that.threads);case SessionUpdate_ThreadUpsert():
return threadUpsert(_that.thread);case SessionUpdate_SyncProgress():
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdate_Kickout():
return kickout(_that.channelId);case SessionUpdate_AuthExpired():
return authExpired(_that.reason);case SessionUpdate_FriendRequest():
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( LinkState state,  String? lastError)?  link,TResult? Function( List<ThreadView> threads)?  inbox,TResult? Function( ThreadView thread)?  threadUpsert,TResult? Function( BigInt pulled,  bool catchingUp)?  syncProgress,TResult? Function( String channelId)?  kickout,TResult? Function( String reason)?  authExpired,TResult? Function( String from,  String nickname)?  friendRequest,TResult? Function( String from,  String nickname)?  friendAccepted,TResult? Function( String account,  String nickname,  String avatar)?  profileUpdated,TResult? Function( String account,  int status,  PlatformInt64 lastSeen)?  presence,TResult? Function( String typer,  String dest,  ThreadKind kind,  bool active)?  typing,TResult? Function( String reader,  String dest,  ThreadKind kind,  PlatformInt64 messageId)?  receiptRead,TResult? Function( String groupId,  List<String> members)?  groupCreate,TResult? Function( List<Person> contacts)?  contactsChanged,TResult? Function( String dest,  AgentTurnState state,  String text)?  agentTurn,TResult? Function( String dest,  AgentCard card)?  agentCard,TResult? Function( String message)?  rustPanic,}) {final _that = this;
switch (_that) {
case SessionUpdate_Link() when link != null:
return link(_that.state,_that.lastError);case SessionUpdate_Inbox() when inbox != null:
return inbox(_that.threads);case SessionUpdate_ThreadUpsert() when threadUpsert != null:
return threadUpsert(_that.thread);case SessionUpdate_SyncProgress() when syncProgress != null:
return syncProgress(_that.pulled,_that.catchingUp);case SessionUpdate_Kickout() when kickout != null:
return kickout(_that.channelId);case SessionUpdate_AuthExpired() when authExpired != null:
return authExpired(_that.reason);case SessionUpdate_FriendRequest() when friendRequest != null:
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
 final  ThreadKind kind;
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
 String typer, String dest, ThreadKind kind, bool active
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
as ThreadKind,active: null == active ? _self.active : active // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class SessionUpdate_ReceiptRead extends SessionUpdate {
  const SessionUpdate_ReceiptRead({required this.reader, required this.dest, required this.kind, required this.messageId}): super._();
  

 final  String reader;
 final  String dest;
 final  ThreadKind kind;
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
 String reader, String dest, ThreadKind kind, PlatformInt64 messageId
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
as ThreadKind,messageId: null == messageId ? _self.messageId : messageId // ignore: cast_nullable_to_non_nullable
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
mixin _$ThreadPreview {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ThreadPreview);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ThreadPreview()';
}


}

/// @nodoc
class $ThreadPreviewCopyWith<$Res>  {
$ThreadPreviewCopyWith(ThreadPreview _, $Res Function(ThreadPreview) __);
}


/// Adds pattern-matching-related methods to [ThreadPreview].
extension ThreadPreviewPatterns on ThreadPreview {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ThreadPreview_Text value)?  text,TResult Function( ThreadPreview_Media value)?  media,TResult Function( ThreadPreview_System value)?  system,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ThreadPreview_Text() when text != null:
return text(_that);case ThreadPreview_Media() when media != null:
return media(_that);case ThreadPreview_System() when system != null:
return system(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ThreadPreview_Text value)  text,required TResult Function( ThreadPreview_Media value)  media,required TResult Function( ThreadPreview_System value)  system,}){
final _that = this;
switch (_that) {
case ThreadPreview_Text():
return text(_that);case ThreadPreview_Media():
return media(_that);case ThreadPreview_System():
return system(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ThreadPreview_Text value)?  text,TResult? Function( ThreadPreview_Media value)?  media,TResult? Function( ThreadPreview_System value)?  system,}){
final _that = this;
switch (_that) {
case ThreadPreview_Text() when text != null:
return text(_that);case ThreadPreview_Media() when media != null:
return media(_that);case ThreadPreview_System() when system != null:
return system(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String snippet)?  text,TResult Function( MediaKind kind)?  media,TResult Function( String text)?  system,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ThreadPreview_Text() when text != null:
return text(_that.snippet);case ThreadPreview_Media() when media != null:
return media(_that.kind);case ThreadPreview_System() when system != null:
return system(_that.text);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String snippet)  text,required TResult Function( MediaKind kind)  media,required TResult Function( String text)  system,}) {final _that = this;
switch (_that) {
case ThreadPreview_Text():
return text(_that.snippet);case ThreadPreview_Media():
return media(_that.kind);case ThreadPreview_System():
return system(_that.text);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String snippet)?  text,TResult? Function( MediaKind kind)?  media,TResult? Function( String text)?  system,}) {final _that = this;
switch (_that) {
case ThreadPreview_Text() when text != null:
return text(_that.snippet);case ThreadPreview_Media() when media != null:
return media(_that.kind);case ThreadPreview_System() when system != null:
return system(_that.text);case _:
  return null;

}
}

}

/// @nodoc


class ThreadPreview_Text extends ThreadPreview {
  const ThreadPreview_Text({required this.snippet}): super._();
  

 final  String snippet;

/// Create a copy of ThreadPreview
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ThreadPreview_TextCopyWith<ThreadPreview_Text> get copyWith => _$ThreadPreview_TextCopyWithImpl<ThreadPreview_Text>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ThreadPreview_Text&&(identical(other.snippet, snippet) || other.snippet == snippet));
}


@override
int get hashCode => Object.hash(runtimeType,snippet);

@override
String toString() {
  return 'ThreadPreview.text(snippet: $snippet)';
}


}

/// @nodoc
abstract mixin class $ThreadPreview_TextCopyWith<$Res> implements $ThreadPreviewCopyWith<$Res> {
  factory $ThreadPreview_TextCopyWith(ThreadPreview_Text value, $Res Function(ThreadPreview_Text) _then) = _$ThreadPreview_TextCopyWithImpl;
@useResult
$Res call({
 String snippet
});




}
/// @nodoc
class _$ThreadPreview_TextCopyWithImpl<$Res>
    implements $ThreadPreview_TextCopyWith<$Res> {
  _$ThreadPreview_TextCopyWithImpl(this._self, this._then);

  final ThreadPreview_Text _self;
  final $Res Function(ThreadPreview_Text) _then;

/// Create a copy of ThreadPreview
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? snippet = null,}) {
  return _then(ThreadPreview_Text(
snippet: null == snippet ? _self.snippet : snippet // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ThreadPreview_Media extends ThreadPreview {
  const ThreadPreview_Media({required this.kind}): super._();
  

 final  MediaKind kind;

/// Create a copy of ThreadPreview
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ThreadPreview_MediaCopyWith<ThreadPreview_Media> get copyWith => _$ThreadPreview_MediaCopyWithImpl<ThreadPreview_Media>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ThreadPreview_Media&&(identical(other.kind, kind) || other.kind == kind));
}


@override
int get hashCode => Object.hash(runtimeType,kind);

@override
String toString() {
  return 'ThreadPreview.media(kind: $kind)';
}


}

/// @nodoc
abstract mixin class $ThreadPreview_MediaCopyWith<$Res> implements $ThreadPreviewCopyWith<$Res> {
  factory $ThreadPreview_MediaCopyWith(ThreadPreview_Media value, $Res Function(ThreadPreview_Media) _then) = _$ThreadPreview_MediaCopyWithImpl;
@useResult
$Res call({
 MediaKind kind
});




}
/// @nodoc
class _$ThreadPreview_MediaCopyWithImpl<$Res>
    implements $ThreadPreview_MediaCopyWith<$Res> {
  _$ThreadPreview_MediaCopyWithImpl(this._self, this._then);

  final ThreadPreview_Media _self;
  final $Res Function(ThreadPreview_Media) _then;

/// Create a copy of ThreadPreview
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? kind = null,}) {
  return _then(ThreadPreview_Media(
kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as MediaKind,
  ));
}


}

/// @nodoc


class ThreadPreview_System extends ThreadPreview {
  const ThreadPreview_System({required this.text}): super._();
  

 final  String text;

/// Create a copy of ThreadPreview
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ThreadPreview_SystemCopyWith<ThreadPreview_System> get copyWith => _$ThreadPreview_SystemCopyWithImpl<ThreadPreview_System>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ThreadPreview_System&&(identical(other.text, text) || other.text == text));
}


@override
int get hashCode => Object.hash(runtimeType,text);

@override
String toString() {
  return 'ThreadPreview.system(text: $text)';
}


}

/// @nodoc
abstract mixin class $ThreadPreview_SystemCopyWith<$Res> implements $ThreadPreviewCopyWith<$Res> {
  factory $ThreadPreview_SystemCopyWith(ThreadPreview_System value, $Res Function(ThreadPreview_System) _then) = _$ThreadPreview_SystemCopyWithImpl;
@useResult
$Res call({
 String text
});




}
/// @nodoc
class _$ThreadPreview_SystemCopyWithImpl<$Res>
    implements $ThreadPreview_SystemCopyWith<$Res> {
  _$ThreadPreview_SystemCopyWithImpl(this._self, this._then);

  final ThreadPreview_System _self;
  final $Res Function(ThreadPreview_System) _then;

/// Create a copy of ThreadPreview
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? text = null,}) {
  return _then(ThreadPreview_System(
text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
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

// dart format on
