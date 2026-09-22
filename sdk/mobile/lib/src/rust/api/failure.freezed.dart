// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'failure.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$ApiFailure {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure()';
}


}

/// @nodoc
class $ApiFailureCopyWith<$Res>  {
$ApiFailureCopyWith(ApiFailure _, $Res Function(ApiFailure) __);
}


/// Adds pattern-matching-related methods to [ApiFailure].
extension ApiFailurePatterns on ApiFailure {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ApiFailure_NotFriends value)?  notFriends,TResult Function( ApiFailure_Blocked value)?  blocked,TResult Function( ApiFailure_UserNotFound value)?  userNotFound,TResult Function( ApiFailure_CannotChatSelf value)?  cannotChatSelf,TResult Function( ApiFailure_AuthExpired value)?  authExpired,TResult Function( ApiFailure_Unauthorized value)?  unauthorized,TResult Function( ApiFailure_InvalidAccount value)?  invalidAccount,TResult Function( ApiFailure_InvalidPassword value)?  invalidPassword,TResult Function( ApiFailure_AccountExists value)?  accountExists,TResult Function( ApiFailure_InsecureOrigin value)?  insecureOrigin,TResult Function( ApiFailure_PasswordSeal value)?  passwordSeal,TResult Function( ApiFailure_AlreadyConnected value)?  alreadyConnected,TResult Function( ApiFailure_NotConnected value)?  notConnected,TResult Function( ApiFailure_StorageFull value)?  storageFull,TResult Function( ApiFailure_SqliteBusy value)?  sqliteBusy,TResult Function( ApiFailure_Disk value)?  disk,TResult Function( ApiFailure_RateLimited value)?  rateLimited,TResult Function( ApiFailure_PayloadTooLarge value)?  payloadTooLarge,TResult Function( ApiFailure_UnsupportedMedia value)?  unsupportedMedia,TResult Function( ApiFailure_Busy value)?  busy,TResult Function( ApiFailure_StaleEpoch value)?  staleEpoch,TResult Function( ApiFailure_NotFound value)?  notFound,TResult Function( ApiFailure_InvalidArgument value)?  invalidArgument,TResult Function( ApiFailure_Protocol value)?  protocol,TResult Function( ApiFailure_Http value)?  http,TResult Function( ApiFailure_Unavailable value)?  unavailable,TResult Function( ApiFailure_Internal value)?  internal,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ApiFailure_NotFriends() when notFriends != null:
return notFriends(_that);case ApiFailure_Blocked() when blocked != null:
return blocked(_that);case ApiFailure_UserNotFound() when userNotFound != null:
return userNotFound(_that);case ApiFailure_CannotChatSelf() when cannotChatSelf != null:
return cannotChatSelf(_that);case ApiFailure_AuthExpired() when authExpired != null:
return authExpired(_that);case ApiFailure_Unauthorized() when unauthorized != null:
return unauthorized(_that);case ApiFailure_InvalidAccount() when invalidAccount != null:
return invalidAccount(_that);case ApiFailure_InvalidPassword() when invalidPassword != null:
return invalidPassword(_that);case ApiFailure_AccountExists() when accountExists != null:
return accountExists(_that);case ApiFailure_InsecureOrigin() when insecureOrigin != null:
return insecureOrigin(_that);case ApiFailure_PasswordSeal() when passwordSeal != null:
return passwordSeal(_that);case ApiFailure_AlreadyConnected() when alreadyConnected != null:
return alreadyConnected(_that);case ApiFailure_NotConnected() when notConnected != null:
return notConnected(_that);case ApiFailure_StorageFull() when storageFull != null:
return storageFull(_that);case ApiFailure_SqliteBusy() when sqliteBusy != null:
return sqliteBusy(_that);case ApiFailure_Disk() when disk != null:
return disk(_that);case ApiFailure_RateLimited() when rateLimited != null:
return rateLimited(_that);case ApiFailure_PayloadTooLarge() when payloadTooLarge != null:
return payloadTooLarge(_that);case ApiFailure_UnsupportedMedia() when unsupportedMedia != null:
return unsupportedMedia(_that);case ApiFailure_Busy() when busy != null:
return busy(_that);case ApiFailure_StaleEpoch() when staleEpoch != null:
return staleEpoch(_that);case ApiFailure_NotFound() when notFound != null:
return notFound(_that);case ApiFailure_InvalidArgument() when invalidArgument != null:
return invalidArgument(_that);case ApiFailure_Protocol() when protocol != null:
return protocol(_that);case ApiFailure_Http() when http != null:
return http(_that);case ApiFailure_Unavailable() when unavailable != null:
return unavailable(_that);case ApiFailure_Internal() when internal != null:
return internal(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ApiFailure_NotFriends value)  notFriends,required TResult Function( ApiFailure_Blocked value)  blocked,required TResult Function( ApiFailure_UserNotFound value)  userNotFound,required TResult Function( ApiFailure_CannotChatSelf value)  cannotChatSelf,required TResult Function( ApiFailure_AuthExpired value)  authExpired,required TResult Function( ApiFailure_Unauthorized value)  unauthorized,required TResult Function( ApiFailure_InvalidAccount value)  invalidAccount,required TResult Function( ApiFailure_InvalidPassword value)  invalidPassword,required TResult Function( ApiFailure_AccountExists value)  accountExists,required TResult Function( ApiFailure_InsecureOrigin value)  insecureOrigin,required TResult Function( ApiFailure_PasswordSeal value)  passwordSeal,required TResult Function( ApiFailure_AlreadyConnected value)  alreadyConnected,required TResult Function( ApiFailure_NotConnected value)  notConnected,required TResult Function( ApiFailure_StorageFull value)  storageFull,required TResult Function( ApiFailure_SqliteBusy value)  sqliteBusy,required TResult Function( ApiFailure_Disk value)  disk,required TResult Function( ApiFailure_RateLimited value)  rateLimited,required TResult Function( ApiFailure_PayloadTooLarge value)  payloadTooLarge,required TResult Function( ApiFailure_UnsupportedMedia value)  unsupportedMedia,required TResult Function( ApiFailure_Busy value)  busy,required TResult Function( ApiFailure_StaleEpoch value)  staleEpoch,required TResult Function( ApiFailure_NotFound value)  notFound,required TResult Function( ApiFailure_InvalidArgument value)  invalidArgument,required TResult Function( ApiFailure_Protocol value)  protocol,required TResult Function( ApiFailure_Http value)  http,required TResult Function( ApiFailure_Unavailable value)  unavailable,required TResult Function( ApiFailure_Internal value)  internal,}){
final _that = this;
switch (_that) {
case ApiFailure_NotFriends():
return notFriends(_that);case ApiFailure_Blocked():
return blocked(_that);case ApiFailure_UserNotFound():
return userNotFound(_that);case ApiFailure_CannotChatSelf():
return cannotChatSelf(_that);case ApiFailure_AuthExpired():
return authExpired(_that);case ApiFailure_Unauthorized():
return unauthorized(_that);case ApiFailure_InvalidAccount():
return invalidAccount(_that);case ApiFailure_InvalidPassword():
return invalidPassword(_that);case ApiFailure_AccountExists():
return accountExists(_that);case ApiFailure_InsecureOrigin():
return insecureOrigin(_that);case ApiFailure_PasswordSeal():
return passwordSeal(_that);case ApiFailure_AlreadyConnected():
return alreadyConnected(_that);case ApiFailure_NotConnected():
return notConnected(_that);case ApiFailure_StorageFull():
return storageFull(_that);case ApiFailure_SqliteBusy():
return sqliteBusy(_that);case ApiFailure_Disk():
return disk(_that);case ApiFailure_RateLimited():
return rateLimited(_that);case ApiFailure_PayloadTooLarge():
return payloadTooLarge(_that);case ApiFailure_UnsupportedMedia():
return unsupportedMedia(_that);case ApiFailure_Busy():
return busy(_that);case ApiFailure_StaleEpoch():
return staleEpoch(_that);case ApiFailure_NotFound():
return notFound(_that);case ApiFailure_InvalidArgument():
return invalidArgument(_that);case ApiFailure_Protocol():
return protocol(_that);case ApiFailure_Http():
return http(_that);case ApiFailure_Unavailable():
return unavailable(_that);case ApiFailure_Internal():
return internal(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ApiFailure_NotFriends value)?  notFriends,TResult? Function( ApiFailure_Blocked value)?  blocked,TResult? Function( ApiFailure_UserNotFound value)?  userNotFound,TResult? Function( ApiFailure_CannotChatSelf value)?  cannotChatSelf,TResult? Function( ApiFailure_AuthExpired value)?  authExpired,TResult? Function( ApiFailure_Unauthorized value)?  unauthorized,TResult? Function( ApiFailure_InvalidAccount value)?  invalidAccount,TResult? Function( ApiFailure_InvalidPassword value)?  invalidPassword,TResult? Function( ApiFailure_AccountExists value)?  accountExists,TResult? Function( ApiFailure_InsecureOrigin value)?  insecureOrigin,TResult? Function( ApiFailure_PasswordSeal value)?  passwordSeal,TResult? Function( ApiFailure_AlreadyConnected value)?  alreadyConnected,TResult? Function( ApiFailure_NotConnected value)?  notConnected,TResult? Function( ApiFailure_StorageFull value)?  storageFull,TResult? Function( ApiFailure_SqliteBusy value)?  sqliteBusy,TResult? Function( ApiFailure_Disk value)?  disk,TResult? Function( ApiFailure_RateLimited value)?  rateLimited,TResult? Function( ApiFailure_PayloadTooLarge value)?  payloadTooLarge,TResult? Function( ApiFailure_UnsupportedMedia value)?  unsupportedMedia,TResult? Function( ApiFailure_Busy value)?  busy,TResult? Function( ApiFailure_StaleEpoch value)?  staleEpoch,TResult? Function( ApiFailure_NotFound value)?  notFound,TResult? Function( ApiFailure_InvalidArgument value)?  invalidArgument,TResult? Function( ApiFailure_Protocol value)?  protocol,TResult? Function( ApiFailure_Http value)?  http,TResult? Function( ApiFailure_Unavailable value)?  unavailable,TResult? Function( ApiFailure_Internal value)?  internal,}){
final _that = this;
switch (_that) {
case ApiFailure_NotFriends() when notFriends != null:
return notFriends(_that);case ApiFailure_Blocked() when blocked != null:
return blocked(_that);case ApiFailure_UserNotFound() when userNotFound != null:
return userNotFound(_that);case ApiFailure_CannotChatSelf() when cannotChatSelf != null:
return cannotChatSelf(_that);case ApiFailure_AuthExpired() when authExpired != null:
return authExpired(_that);case ApiFailure_Unauthorized() when unauthorized != null:
return unauthorized(_that);case ApiFailure_InvalidAccount() when invalidAccount != null:
return invalidAccount(_that);case ApiFailure_InvalidPassword() when invalidPassword != null:
return invalidPassword(_that);case ApiFailure_AccountExists() when accountExists != null:
return accountExists(_that);case ApiFailure_InsecureOrigin() when insecureOrigin != null:
return insecureOrigin(_that);case ApiFailure_PasswordSeal() when passwordSeal != null:
return passwordSeal(_that);case ApiFailure_AlreadyConnected() when alreadyConnected != null:
return alreadyConnected(_that);case ApiFailure_NotConnected() when notConnected != null:
return notConnected(_that);case ApiFailure_StorageFull() when storageFull != null:
return storageFull(_that);case ApiFailure_SqliteBusy() when sqliteBusy != null:
return sqliteBusy(_that);case ApiFailure_Disk() when disk != null:
return disk(_that);case ApiFailure_RateLimited() when rateLimited != null:
return rateLimited(_that);case ApiFailure_PayloadTooLarge() when payloadTooLarge != null:
return payloadTooLarge(_that);case ApiFailure_UnsupportedMedia() when unsupportedMedia != null:
return unsupportedMedia(_that);case ApiFailure_Busy() when busy != null:
return busy(_that);case ApiFailure_StaleEpoch() when staleEpoch != null:
return staleEpoch(_that);case ApiFailure_NotFound() when notFound != null:
return notFound(_that);case ApiFailure_InvalidArgument() when invalidArgument != null:
return invalidArgument(_that);case ApiFailure_Protocol() when protocol != null:
return protocol(_that);case ApiFailure_Http() when http != null:
return http(_that);case ApiFailure_Unavailable() when unavailable != null:
return unavailable(_that);case ApiFailure_Internal() when internal != null:
return internal(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String dest)?  notFriends,TResult Function( String dest)?  blocked,TResult Function( String dest)?  userNotFound,TResult Function()?  cannotChatSelf,TResult Function()?  authExpired,TResult Function()?  unauthorized,TResult Function()?  invalidAccount,TResult Function()?  invalidPassword,TResult Function()?  accountExists,TResult Function()?  insecureOrigin,TResult Function()?  passwordSeal,TResult Function()?  alreadyConnected,TResult Function()?  notConnected,TResult Function()?  storageFull,TResult Function()?  sqliteBusy,TResult Function( String message)?  disk,TResult Function( PlatformInt64 retryAfterMs)?  rateLimited,TResult Function( PlatformInt64 bytes,  PlatformInt64 max)?  payloadTooLarge,TResult Function( String mime)?  unsupportedMedia,TResult Function( String queue)?  busy,TResult Function( BigInt expected,  BigInt actual)?  staleEpoch,TResult Function( String what)?  notFound,TResult Function( String message)?  invalidArgument,TResult Function( int status)?  protocol,TResult Function( int status)?  http,TResult Function( String what)?  unavailable,TResult Function( String message)?  internal,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ApiFailure_NotFriends() when notFriends != null:
return notFriends(_that.dest);case ApiFailure_Blocked() when blocked != null:
return blocked(_that.dest);case ApiFailure_UserNotFound() when userNotFound != null:
return userNotFound(_that.dest);case ApiFailure_CannotChatSelf() when cannotChatSelf != null:
return cannotChatSelf();case ApiFailure_AuthExpired() when authExpired != null:
return authExpired();case ApiFailure_Unauthorized() when unauthorized != null:
return unauthorized();case ApiFailure_InvalidAccount() when invalidAccount != null:
return invalidAccount();case ApiFailure_InvalidPassword() when invalidPassword != null:
return invalidPassword();case ApiFailure_AccountExists() when accountExists != null:
return accountExists();case ApiFailure_InsecureOrigin() when insecureOrigin != null:
return insecureOrigin();case ApiFailure_PasswordSeal() when passwordSeal != null:
return passwordSeal();case ApiFailure_AlreadyConnected() when alreadyConnected != null:
return alreadyConnected();case ApiFailure_NotConnected() when notConnected != null:
return notConnected();case ApiFailure_StorageFull() when storageFull != null:
return storageFull();case ApiFailure_SqliteBusy() when sqliteBusy != null:
return sqliteBusy();case ApiFailure_Disk() when disk != null:
return disk(_that.message);case ApiFailure_RateLimited() when rateLimited != null:
return rateLimited(_that.retryAfterMs);case ApiFailure_PayloadTooLarge() when payloadTooLarge != null:
return payloadTooLarge(_that.bytes,_that.max);case ApiFailure_UnsupportedMedia() when unsupportedMedia != null:
return unsupportedMedia(_that.mime);case ApiFailure_Busy() when busy != null:
return busy(_that.queue);case ApiFailure_StaleEpoch() when staleEpoch != null:
return staleEpoch(_that.expected,_that.actual);case ApiFailure_NotFound() when notFound != null:
return notFound(_that.what);case ApiFailure_InvalidArgument() when invalidArgument != null:
return invalidArgument(_that.message);case ApiFailure_Protocol() when protocol != null:
return protocol(_that.status);case ApiFailure_Http() when http != null:
return http(_that.status);case ApiFailure_Unavailable() when unavailable != null:
return unavailable(_that.what);case ApiFailure_Internal() when internal != null:
return internal(_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String dest)  notFriends,required TResult Function( String dest)  blocked,required TResult Function( String dest)  userNotFound,required TResult Function()  cannotChatSelf,required TResult Function()  authExpired,required TResult Function()  unauthorized,required TResult Function()  invalidAccount,required TResult Function()  invalidPassword,required TResult Function()  accountExists,required TResult Function()  insecureOrigin,required TResult Function()  passwordSeal,required TResult Function()  alreadyConnected,required TResult Function()  notConnected,required TResult Function()  storageFull,required TResult Function()  sqliteBusy,required TResult Function( String message)  disk,required TResult Function( PlatformInt64 retryAfterMs)  rateLimited,required TResult Function( PlatformInt64 bytes,  PlatformInt64 max)  payloadTooLarge,required TResult Function( String mime)  unsupportedMedia,required TResult Function( String queue)  busy,required TResult Function( BigInt expected,  BigInt actual)  staleEpoch,required TResult Function( String what)  notFound,required TResult Function( String message)  invalidArgument,required TResult Function( int status)  protocol,required TResult Function( int status)  http,required TResult Function( String what)  unavailable,required TResult Function( String message)  internal,}) {final _that = this;
switch (_that) {
case ApiFailure_NotFriends():
return notFriends(_that.dest);case ApiFailure_Blocked():
return blocked(_that.dest);case ApiFailure_UserNotFound():
return userNotFound(_that.dest);case ApiFailure_CannotChatSelf():
return cannotChatSelf();case ApiFailure_AuthExpired():
return authExpired();case ApiFailure_Unauthorized():
return unauthorized();case ApiFailure_InvalidAccount():
return invalidAccount();case ApiFailure_InvalidPassword():
return invalidPassword();case ApiFailure_AccountExists():
return accountExists();case ApiFailure_InsecureOrigin():
return insecureOrigin();case ApiFailure_PasswordSeal():
return passwordSeal();case ApiFailure_AlreadyConnected():
return alreadyConnected();case ApiFailure_NotConnected():
return notConnected();case ApiFailure_StorageFull():
return storageFull();case ApiFailure_SqliteBusy():
return sqliteBusy();case ApiFailure_Disk():
return disk(_that.message);case ApiFailure_RateLimited():
return rateLimited(_that.retryAfterMs);case ApiFailure_PayloadTooLarge():
return payloadTooLarge(_that.bytes,_that.max);case ApiFailure_UnsupportedMedia():
return unsupportedMedia(_that.mime);case ApiFailure_Busy():
return busy(_that.queue);case ApiFailure_StaleEpoch():
return staleEpoch(_that.expected,_that.actual);case ApiFailure_NotFound():
return notFound(_that.what);case ApiFailure_InvalidArgument():
return invalidArgument(_that.message);case ApiFailure_Protocol():
return protocol(_that.status);case ApiFailure_Http():
return http(_that.status);case ApiFailure_Unavailable():
return unavailable(_that.what);case ApiFailure_Internal():
return internal(_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String dest)?  notFriends,TResult? Function( String dest)?  blocked,TResult? Function( String dest)?  userNotFound,TResult? Function()?  cannotChatSelf,TResult? Function()?  authExpired,TResult? Function()?  unauthorized,TResult? Function()?  invalidAccount,TResult? Function()?  invalidPassword,TResult? Function()?  accountExists,TResult? Function()?  insecureOrigin,TResult? Function()?  passwordSeal,TResult? Function()?  alreadyConnected,TResult? Function()?  notConnected,TResult? Function()?  storageFull,TResult? Function()?  sqliteBusy,TResult? Function( String message)?  disk,TResult? Function( PlatformInt64 retryAfterMs)?  rateLimited,TResult? Function( PlatformInt64 bytes,  PlatformInt64 max)?  payloadTooLarge,TResult? Function( String mime)?  unsupportedMedia,TResult? Function( String queue)?  busy,TResult? Function( BigInt expected,  BigInt actual)?  staleEpoch,TResult? Function( String what)?  notFound,TResult? Function( String message)?  invalidArgument,TResult? Function( int status)?  protocol,TResult? Function( int status)?  http,TResult? Function( String what)?  unavailable,TResult? Function( String message)?  internal,}) {final _that = this;
switch (_that) {
case ApiFailure_NotFriends() when notFriends != null:
return notFriends(_that.dest);case ApiFailure_Blocked() when blocked != null:
return blocked(_that.dest);case ApiFailure_UserNotFound() when userNotFound != null:
return userNotFound(_that.dest);case ApiFailure_CannotChatSelf() when cannotChatSelf != null:
return cannotChatSelf();case ApiFailure_AuthExpired() when authExpired != null:
return authExpired();case ApiFailure_Unauthorized() when unauthorized != null:
return unauthorized();case ApiFailure_InvalidAccount() when invalidAccount != null:
return invalidAccount();case ApiFailure_InvalidPassword() when invalidPassword != null:
return invalidPassword();case ApiFailure_AccountExists() when accountExists != null:
return accountExists();case ApiFailure_InsecureOrigin() when insecureOrigin != null:
return insecureOrigin();case ApiFailure_PasswordSeal() when passwordSeal != null:
return passwordSeal();case ApiFailure_AlreadyConnected() when alreadyConnected != null:
return alreadyConnected();case ApiFailure_NotConnected() when notConnected != null:
return notConnected();case ApiFailure_StorageFull() when storageFull != null:
return storageFull();case ApiFailure_SqliteBusy() when sqliteBusy != null:
return sqliteBusy();case ApiFailure_Disk() when disk != null:
return disk(_that.message);case ApiFailure_RateLimited() when rateLimited != null:
return rateLimited(_that.retryAfterMs);case ApiFailure_PayloadTooLarge() when payloadTooLarge != null:
return payloadTooLarge(_that.bytes,_that.max);case ApiFailure_UnsupportedMedia() when unsupportedMedia != null:
return unsupportedMedia(_that.mime);case ApiFailure_Busy() when busy != null:
return busy(_that.queue);case ApiFailure_StaleEpoch() when staleEpoch != null:
return staleEpoch(_that.expected,_that.actual);case ApiFailure_NotFound() when notFound != null:
return notFound(_that.what);case ApiFailure_InvalidArgument() when invalidArgument != null:
return invalidArgument(_that.message);case ApiFailure_Protocol() when protocol != null:
return protocol(_that.status);case ApiFailure_Http() when http != null:
return http(_that.status);case ApiFailure_Unavailable() when unavailable != null:
return unavailable(_that.what);case ApiFailure_Internal() when internal != null:
return internal(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class ApiFailure_NotFriends extends ApiFailure {
  const ApiFailure_NotFriends({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_NotFriendsCopyWith<ApiFailure_NotFriends> get copyWith => _$ApiFailure_NotFriendsCopyWithImpl<ApiFailure_NotFriends>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_NotFriends&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'ApiFailure.notFriends(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_NotFriendsCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_NotFriendsCopyWith(ApiFailure_NotFriends value, $Res Function(ApiFailure_NotFriends) _then) = _$ApiFailure_NotFriendsCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$ApiFailure_NotFriendsCopyWithImpl<$Res>
    implements $ApiFailure_NotFriendsCopyWith<$Res> {
  _$ApiFailure_NotFriendsCopyWithImpl(this._self, this._then);

  final ApiFailure_NotFriends _self;
  final $Res Function(ApiFailure_NotFriends) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(ApiFailure_NotFriends(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_Blocked extends ApiFailure {
  const ApiFailure_Blocked({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_BlockedCopyWith<ApiFailure_Blocked> get copyWith => _$ApiFailure_BlockedCopyWithImpl<ApiFailure_Blocked>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_Blocked&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'ApiFailure.blocked(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_BlockedCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_BlockedCopyWith(ApiFailure_Blocked value, $Res Function(ApiFailure_Blocked) _then) = _$ApiFailure_BlockedCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$ApiFailure_BlockedCopyWithImpl<$Res>
    implements $ApiFailure_BlockedCopyWith<$Res> {
  _$ApiFailure_BlockedCopyWithImpl(this._self, this._then);

  final ApiFailure_Blocked _self;
  final $Res Function(ApiFailure_Blocked) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(ApiFailure_Blocked(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_UserNotFound extends ApiFailure {
  const ApiFailure_UserNotFound({required this.dest}): super._();
  

 final  String dest;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_UserNotFoundCopyWith<ApiFailure_UserNotFound> get copyWith => _$ApiFailure_UserNotFoundCopyWithImpl<ApiFailure_UserNotFound>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_UserNotFound&&(identical(other.dest, dest) || other.dest == dest));
}


@override
int get hashCode => Object.hash(runtimeType,dest);

@override
String toString() {
  return 'ApiFailure.userNotFound(dest: $dest)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_UserNotFoundCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_UserNotFoundCopyWith(ApiFailure_UserNotFound value, $Res Function(ApiFailure_UserNotFound) _then) = _$ApiFailure_UserNotFoundCopyWithImpl;
@useResult
$Res call({
 String dest
});




}
/// @nodoc
class _$ApiFailure_UserNotFoundCopyWithImpl<$Res>
    implements $ApiFailure_UserNotFoundCopyWith<$Res> {
  _$ApiFailure_UserNotFoundCopyWithImpl(this._self, this._then);

  final ApiFailure_UserNotFound _self;
  final $Res Function(ApiFailure_UserNotFound) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? dest = null,}) {
  return _then(ApiFailure_UserNotFound(
dest: null == dest ? _self.dest : dest // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_CannotChatSelf extends ApiFailure {
  const ApiFailure_CannotChatSelf(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_CannotChatSelf);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.cannotChatSelf()';
}


}




/// @nodoc


class ApiFailure_AuthExpired extends ApiFailure {
  const ApiFailure_AuthExpired(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_AuthExpired);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.authExpired()';
}


}




/// @nodoc


class ApiFailure_Unauthorized extends ApiFailure {
  const ApiFailure_Unauthorized(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_Unauthorized);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.unauthorized()';
}


}




/// @nodoc


class ApiFailure_InvalidAccount extends ApiFailure {
  const ApiFailure_InvalidAccount(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_InvalidAccount);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.invalidAccount()';
}


}




/// @nodoc


class ApiFailure_InvalidPassword extends ApiFailure {
  const ApiFailure_InvalidPassword(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_InvalidPassword);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.invalidPassword()';
}


}




/// @nodoc


class ApiFailure_AccountExists extends ApiFailure {
  const ApiFailure_AccountExists(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_AccountExists);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.accountExists()';
}


}




/// @nodoc


class ApiFailure_InsecureOrigin extends ApiFailure {
  const ApiFailure_InsecureOrigin(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_InsecureOrigin);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.insecureOrigin()';
}


}




/// @nodoc


class ApiFailure_PasswordSeal extends ApiFailure {
  const ApiFailure_PasswordSeal(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_PasswordSeal);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.passwordSeal()';
}


}




/// @nodoc


class ApiFailure_AlreadyConnected extends ApiFailure {
  const ApiFailure_AlreadyConnected(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_AlreadyConnected);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.alreadyConnected()';
}


}




/// @nodoc


class ApiFailure_NotConnected extends ApiFailure {
  const ApiFailure_NotConnected(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_NotConnected);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.notConnected()';
}


}




/// @nodoc


class ApiFailure_StorageFull extends ApiFailure {
  const ApiFailure_StorageFull(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_StorageFull);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.storageFull()';
}


}




/// @nodoc


class ApiFailure_SqliteBusy extends ApiFailure {
  const ApiFailure_SqliteBusy(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_SqliteBusy);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ApiFailure.sqliteBusy()';
}


}




/// @nodoc


class ApiFailure_Disk extends ApiFailure {
  const ApiFailure_Disk({required this.message}): super._();
  

 final  String message;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_DiskCopyWith<ApiFailure_Disk> get copyWith => _$ApiFailure_DiskCopyWithImpl<ApiFailure_Disk>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_Disk&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'ApiFailure.disk(message: $message)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_DiskCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_DiskCopyWith(ApiFailure_Disk value, $Res Function(ApiFailure_Disk) _then) = _$ApiFailure_DiskCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$ApiFailure_DiskCopyWithImpl<$Res>
    implements $ApiFailure_DiskCopyWith<$Res> {
  _$ApiFailure_DiskCopyWithImpl(this._self, this._then);

  final ApiFailure_Disk _self;
  final $Res Function(ApiFailure_Disk) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(ApiFailure_Disk(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_RateLimited extends ApiFailure {
  const ApiFailure_RateLimited({required this.retryAfterMs}): super._();
  

 final  PlatformInt64 retryAfterMs;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_RateLimitedCopyWith<ApiFailure_RateLimited> get copyWith => _$ApiFailure_RateLimitedCopyWithImpl<ApiFailure_RateLimited>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_RateLimited&&(identical(other.retryAfterMs, retryAfterMs) || other.retryAfterMs == retryAfterMs));
}


@override
int get hashCode => Object.hash(runtimeType,retryAfterMs);

@override
String toString() {
  return 'ApiFailure.rateLimited(retryAfterMs: $retryAfterMs)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_RateLimitedCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_RateLimitedCopyWith(ApiFailure_RateLimited value, $Res Function(ApiFailure_RateLimited) _then) = _$ApiFailure_RateLimitedCopyWithImpl;
@useResult
$Res call({
 PlatformInt64 retryAfterMs
});




}
/// @nodoc
class _$ApiFailure_RateLimitedCopyWithImpl<$Res>
    implements $ApiFailure_RateLimitedCopyWith<$Res> {
  _$ApiFailure_RateLimitedCopyWithImpl(this._self, this._then);

  final ApiFailure_RateLimited _self;
  final $Res Function(ApiFailure_RateLimited) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? retryAfterMs = null,}) {
  return _then(ApiFailure_RateLimited(
retryAfterMs: null == retryAfterMs ? _self.retryAfterMs : retryAfterMs // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class ApiFailure_PayloadTooLarge extends ApiFailure {
  const ApiFailure_PayloadTooLarge({required this.bytes, required this.max}): super._();
  

 final  PlatformInt64 bytes;
 final  PlatformInt64 max;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_PayloadTooLargeCopyWith<ApiFailure_PayloadTooLarge> get copyWith => _$ApiFailure_PayloadTooLargeCopyWithImpl<ApiFailure_PayloadTooLarge>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_PayloadTooLarge&&(identical(other.bytes, bytes) || other.bytes == bytes)&&(identical(other.max, max) || other.max == max));
}


@override
int get hashCode => Object.hash(runtimeType,bytes,max);

@override
String toString() {
  return 'ApiFailure.payloadTooLarge(bytes: $bytes, max: $max)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_PayloadTooLargeCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_PayloadTooLargeCopyWith(ApiFailure_PayloadTooLarge value, $Res Function(ApiFailure_PayloadTooLarge) _then) = _$ApiFailure_PayloadTooLargeCopyWithImpl;
@useResult
$Res call({
 PlatformInt64 bytes, PlatformInt64 max
});




}
/// @nodoc
class _$ApiFailure_PayloadTooLargeCopyWithImpl<$Res>
    implements $ApiFailure_PayloadTooLargeCopyWith<$Res> {
  _$ApiFailure_PayloadTooLargeCopyWithImpl(this._self, this._then);

  final ApiFailure_PayloadTooLarge _self;
  final $Res Function(ApiFailure_PayloadTooLarge) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? bytes = null,Object? max = null,}) {
  return _then(ApiFailure_PayloadTooLarge(
bytes: null == bytes ? _self.bytes : bytes // ignore: cast_nullable_to_non_nullable
as PlatformInt64,max: null == max ? _self.max : max // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class ApiFailure_UnsupportedMedia extends ApiFailure {
  const ApiFailure_UnsupportedMedia({required this.mime}): super._();
  

 final  String mime;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_UnsupportedMediaCopyWith<ApiFailure_UnsupportedMedia> get copyWith => _$ApiFailure_UnsupportedMediaCopyWithImpl<ApiFailure_UnsupportedMedia>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_UnsupportedMedia&&(identical(other.mime, mime) || other.mime == mime));
}


@override
int get hashCode => Object.hash(runtimeType,mime);

@override
String toString() {
  return 'ApiFailure.unsupportedMedia(mime: $mime)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_UnsupportedMediaCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_UnsupportedMediaCopyWith(ApiFailure_UnsupportedMedia value, $Res Function(ApiFailure_UnsupportedMedia) _then) = _$ApiFailure_UnsupportedMediaCopyWithImpl;
@useResult
$Res call({
 String mime
});




}
/// @nodoc
class _$ApiFailure_UnsupportedMediaCopyWithImpl<$Res>
    implements $ApiFailure_UnsupportedMediaCopyWith<$Res> {
  _$ApiFailure_UnsupportedMediaCopyWithImpl(this._self, this._then);

  final ApiFailure_UnsupportedMedia _self;
  final $Res Function(ApiFailure_UnsupportedMedia) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? mime = null,}) {
  return _then(ApiFailure_UnsupportedMedia(
mime: null == mime ? _self.mime : mime // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_Busy extends ApiFailure {
  const ApiFailure_Busy({required this.queue}): super._();
  

 final  String queue;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_BusyCopyWith<ApiFailure_Busy> get copyWith => _$ApiFailure_BusyCopyWithImpl<ApiFailure_Busy>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_Busy&&(identical(other.queue, queue) || other.queue == queue));
}


@override
int get hashCode => Object.hash(runtimeType,queue);

@override
String toString() {
  return 'ApiFailure.busy(queue: $queue)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_BusyCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_BusyCopyWith(ApiFailure_Busy value, $Res Function(ApiFailure_Busy) _then) = _$ApiFailure_BusyCopyWithImpl;
@useResult
$Res call({
 String queue
});




}
/// @nodoc
class _$ApiFailure_BusyCopyWithImpl<$Res>
    implements $ApiFailure_BusyCopyWith<$Res> {
  _$ApiFailure_BusyCopyWithImpl(this._self, this._then);

  final ApiFailure_Busy _self;
  final $Res Function(ApiFailure_Busy) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? queue = null,}) {
  return _then(ApiFailure_Busy(
queue: null == queue ? _self.queue : queue // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_StaleEpoch extends ApiFailure {
  const ApiFailure_StaleEpoch({required this.expected, required this.actual}): super._();
  

 final  BigInt expected;
 final  BigInt actual;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_StaleEpochCopyWith<ApiFailure_StaleEpoch> get copyWith => _$ApiFailure_StaleEpochCopyWithImpl<ApiFailure_StaleEpoch>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_StaleEpoch&&(identical(other.expected, expected) || other.expected == expected)&&(identical(other.actual, actual) || other.actual == actual));
}


@override
int get hashCode => Object.hash(runtimeType,expected,actual);

@override
String toString() {
  return 'ApiFailure.staleEpoch(expected: $expected, actual: $actual)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_StaleEpochCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_StaleEpochCopyWith(ApiFailure_StaleEpoch value, $Res Function(ApiFailure_StaleEpoch) _then) = _$ApiFailure_StaleEpochCopyWithImpl;
@useResult
$Res call({
 BigInt expected, BigInt actual
});




}
/// @nodoc
class _$ApiFailure_StaleEpochCopyWithImpl<$Res>
    implements $ApiFailure_StaleEpochCopyWith<$Res> {
  _$ApiFailure_StaleEpochCopyWithImpl(this._self, this._then);

  final ApiFailure_StaleEpoch _self;
  final $Res Function(ApiFailure_StaleEpoch) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? expected = null,Object? actual = null,}) {
  return _then(ApiFailure_StaleEpoch(
expected: null == expected ? _self.expected : expected // ignore: cast_nullable_to_non_nullable
as BigInt,actual: null == actual ? _self.actual : actual // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ApiFailure_NotFound extends ApiFailure {
  const ApiFailure_NotFound({required this.what}): super._();
  

 final  String what;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_NotFoundCopyWith<ApiFailure_NotFound> get copyWith => _$ApiFailure_NotFoundCopyWithImpl<ApiFailure_NotFound>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_NotFound&&(identical(other.what, what) || other.what == what));
}


@override
int get hashCode => Object.hash(runtimeType,what);

@override
String toString() {
  return 'ApiFailure.notFound(what: $what)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_NotFoundCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_NotFoundCopyWith(ApiFailure_NotFound value, $Res Function(ApiFailure_NotFound) _then) = _$ApiFailure_NotFoundCopyWithImpl;
@useResult
$Res call({
 String what
});




}
/// @nodoc
class _$ApiFailure_NotFoundCopyWithImpl<$Res>
    implements $ApiFailure_NotFoundCopyWith<$Res> {
  _$ApiFailure_NotFoundCopyWithImpl(this._self, this._then);

  final ApiFailure_NotFound _self;
  final $Res Function(ApiFailure_NotFound) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? what = null,}) {
  return _then(ApiFailure_NotFound(
what: null == what ? _self.what : what // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_InvalidArgument extends ApiFailure {
  const ApiFailure_InvalidArgument({required this.message}): super._();
  

 final  String message;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_InvalidArgumentCopyWith<ApiFailure_InvalidArgument> get copyWith => _$ApiFailure_InvalidArgumentCopyWithImpl<ApiFailure_InvalidArgument>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_InvalidArgument&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'ApiFailure.invalidArgument(message: $message)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_InvalidArgumentCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_InvalidArgumentCopyWith(ApiFailure_InvalidArgument value, $Res Function(ApiFailure_InvalidArgument) _then) = _$ApiFailure_InvalidArgumentCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$ApiFailure_InvalidArgumentCopyWithImpl<$Res>
    implements $ApiFailure_InvalidArgumentCopyWith<$Res> {
  _$ApiFailure_InvalidArgumentCopyWithImpl(this._self, this._then);

  final ApiFailure_InvalidArgument _self;
  final $Res Function(ApiFailure_InvalidArgument) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(ApiFailure_InvalidArgument(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_Protocol extends ApiFailure {
  const ApiFailure_Protocol({required this.status}): super._();
  

 final  int status;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_ProtocolCopyWith<ApiFailure_Protocol> get copyWith => _$ApiFailure_ProtocolCopyWithImpl<ApiFailure_Protocol>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_Protocol&&(identical(other.status, status) || other.status == status));
}


@override
int get hashCode => Object.hash(runtimeType,status);

@override
String toString() {
  return 'ApiFailure.protocol(status: $status)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_ProtocolCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_ProtocolCopyWith(ApiFailure_Protocol value, $Res Function(ApiFailure_Protocol) _then) = _$ApiFailure_ProtocolCopyWithImpl;
@useResult
$Res call({
 int status
});




}
/// @nodoc
class _$ApiFailure_ProtocolCopyWithImpl<$Res>
    implements $ApiFailure_ProtocolCopyWith<$Res> {
  _$ApiFailure_ProtocolCopyWithImpl(this._self, this._then);

  final ApiFailure_Protocol _self;
  final $Res Function(ApiFailure_Protocol) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? status = null,}) {
  return _then(ApiFailure_Protocol(
status: null == status ? _self.status : status // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class ApiFailure_Http extends ApiFailure {
  const ApiFailure_Http({required this.status}): super._();
  

 final  int status;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_HttpCopyWith<ApiFailure_Http> get copyWith => _$ApiFailure_HttpCopyWithImpl<ApiFailure_Http>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_Http&&(identical(other.status, status) || other.status == status));
}


@override
int get hashCode => Object.hash(runtimeType,status);

@override
String toString() {
  return 'ApiFailure.http(status: $status)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_HttpCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_HttpCopyWith(ApiFailure_Http value, $Res Function(ApiFailure_Http) _then) = _$ApiFailure_HttpCopyWithImpl;
@useResult
$Res call({
 int status
});




}
/// @nodoc
class _$ApiFailure_HttpCopyWithImpl<$Res>
    implements $ApiFailure_HttpCopyWith<$Res> {
  _$ApiFailure_HttpCopyWithImpl(this._self, this._then);

  final ApiFailure_Http _self;
  final $Res Function(ApiFailure_Http) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? status = null,}) {
  return _then(ApiFailure_Http(
status: null == status ? _self.status : status // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class ApiFailure_Unavailable extends ApiFailure {
  const ApiFailure_Unavailable({required this.what}): super._();
  

 final  String what;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_UnavailableCopyWith<ApiFailure_Unavailable> get copyWith => _$ApiFailure_UnavailableCopyWithImpl<ApiFailure_Unavailable>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_Unavailable&&(identical(other.what, what) || other.what == what));
}


@override
int get hashCode => Object.hash(runtimeType,what);

@override
String toString() {
  return 'ApiFailure.unavailable(what: $what)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_UnavailableCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_UnavailableCopyWith(ApiFailure_Unavailable value, $Res Function(ApiFailure_Unavailable) _then) = _$ApiFailure_UnavailableCopyWithImpl;
@useResult
$Res call({
 String what
});




}
/// @nodoc
class _$ApiFailure_UnavailableCopyWithImpl<$Res>
    implements $ApiFailure_UnavailableCopyWith<$Res> {
  _$ApiFailure_UnavailableCopyWithImpl(this._self, this._then);

  final ApiFailure_Unavailable _self;
  final $Res Function(ApiFailure_Unavailable) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? what = null,}) {
  return _then(ApiFailure_Unavailable(
what: null == what ? _self.what : what // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ApiFailure_Internal extends ApiFailure {
  const ApiFailure_Internal({required this.message}): super._();
  

 final  String message;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ApiFailure_InternalCopyWith<ApiFailure_Internal> get copyWith => _$ApiFailure_InternalCopyWithImpl<ApiFailure_Internal>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ApiFailure_Internal&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'ApiFailure.internal(message: $message)';
}


}

/// @nodoc
abstract mixin class $ApiFailure_InternalCopyWith<$Res> implements $ApiFailureCopyWith<$Res> {
  factory $ApiFailure_InternalCopyWith(ApiFailure_Internal value, $Res Function(ApiFailure_Internal) _then) = _$ApiFailure_InternalCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$ApiFailure_InternalCopyWithImpl<$Res>
    implements $ApiFailure_InternalCopyWith<$Res> {
  _$ApiFailure_InternalCopyWithImpl(this._self, this._then);

  final ApiFailure_Internal _self;
  final $Res Function(ApiFailure_Internal) _then;

/// Create a copy of ApiFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(ApiFailure_Internal(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
