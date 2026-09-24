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
mixin _$AgentFailure {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'AgentFailure()';
}


}

/// @nodoc
class $AgentFailureCopyWith<$Res>  {
$AgentFailureCopyWith(AgentFailure _, $Res Function(AgentFailure) __);
}


/// Adds pattern-matching-related methods to [AgentFailure].
extension AgentFailurePatterns on AgentFailure {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( AgentFailure_MissingApiKey value)?  missingApiKey,TResult Function( AgentFailure_UnknownProvider value)?  unknownProvider,TResult Function( AgentFailure_InvalidUrl value)?  invalidUrl,TResult Function( AgentFailure_Busy value)?  busy,TResult Function( AgentFailure_UnknownSession value)?  unknownSession,TResult Function( AgentFailure_UnknownToolCall value)?  unknownToolCall,TResult Function( AgentFailure_RateLimited value)?  rateLimited,TResult Function( AgentFailure_ContextExceeded value)?  contextExceeded,TResult Function( AgentFailure_Poisoned value)?  poisoned,TResult Function( AgentFailure_Failed value)?  failed,required TResult orElse(),}){
final _that = this;
switch (_that) {
case AgentFailure_MissingApiKey() when missingApiKey != null:
return missingApiKey(_that);case AgentFailure_UnknownProvider() when unknownProvider != null:
return unknownProvider(_that);case AgentFailure_InvalidUrl() when invalidUrl != null:
return invalidUrl(_that);case AgentFailure_Busy() when busy != null:
return busy(_that);case AgentFailure_UnknownSession() when unknownSession != null:
return unknownSession(_that);case AgentFailure_UnknownToolCall() when unknownToolCall != null:
return unknownToolCall(_that);case AgentFailure_RateLimited() when rateLimited != null:
return rateLimited(_that);case AgentFailure_ContextExceeded() when contextExceeded != null:
return contextExceeded(_that);case AgentFailure_Poisoned() when poisoned != null:
return poisoned(_that);case AgentFailure_Failed() when failed != null:
return failed(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( AgentFailure_MissingApiKey value)  missingApiKey,required TResult Function( AgentFailure_UnknownProvider value)  unknownProvider,required TResult Function( AgentFailure_InvalidUrl value)  invalidUrl,required TResult Function( AgentFailure_Busy value)  busy,required TResult Function( AgentFailure_UnknownSession value)  unknownSession,required TResult Function( AgentFailure_UnknownToolCall value)  unknownToolCall,required TResult Function( AgentFailure_RateLimited value)  rateLimited,required TResult Function( AgentFailure_ContextExceeded value)  contextExceeded,required TResult Function( AgentFailure_Poisoned value)  poisoned,required TResult Function( AgentFailure_Failed value)  failed,}){
final _that = this;
switch (_that) {
case AgentFailure_MissingApiKey():
return missingApiKey(_that);case AgentFailure_UnknownProvider():
return unknownProvider(_that);case AgentFailure_InvalidUrl():
return invalidUrl(_that);case AgentFailure_Busy():
return busy(_that);case AgentFailure_UnknownSession():
return unknownSession(_that);case AgentFailure_UnknownToolCall():
return unknownToolCall(_that);case AgentFailure_RateLimited():
return rateLimited(_that);case AgentFailure_ContextExceeded():
return contextExceeded(_that);case AgentFailure_Poisoned():
return poisoned(_that);case AgentFailure_Failed():
return failed(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( AgentFailure_MissingApiKey value)?  missingApiKey,TResult? Function( AgentFailure_UnknownProvider value)?  unknownProvider,TResult? Function( AgentFailure_InvalidUrl value)?  invalidUrl,TResult? Function( AgentFailure_Busy value)?  busy,TResult? Function( AgentFailure_UnknownSession value)?  unknownSession,TResult? Function( AgentFailure_UnknownToolCall value)?  unknownToolCall,TResult? Function( AgentFailure_RateLimited value)?  rateLimited,TResult? Function( AgentFailure_ContextExceeded value)?  contextExceeded,TResult? Function( AgentFailure_Poisoned value)?  poisoned,TResult? Function( AgentFailure_Failed value)?  failed,}){
final _that = this;
switch (_that) {
case AgentFailure_MissingApiKey() when missingApiKey != null:
return missingApiKey(_that);case AgentFailure_UnknownProvider() when unknownProvider != null:
return unknownProvider(_that);case AgentFailure_InvalidUrl() when invalidUrl != null:
return invalidUrl(_that);case AgentFailure_Busy() when busy != null:
return busy(_that);case AgentFailure_UnknownSession() when unknownSession != null:
return unknownSession(_that);case AgentFailure_UnknownToolCall() when unknownToolCall != null:
return unknownToolCall(_that);case AgentFailure_RateLimited() when rateLimited != null:
return rateLimited(_that);case AgentFailure_ContextExceeded() when contextExceeded != null:
return contextExceeded(_that);case AgentFailure_Poisoned() when poisoned != null:
return poisoned(_that);case AgentFailure_Failed() when failed != null:
return failed(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  missingApiKey,TResult Function( String name)?  unknownProvider,TResult Function()?  invalidUrl,TResult Function()?  busy,TResult Function()?  unknownSession,TResult Function()?  unknownToolCall,TResult Function()?  rateLimited,TResult Function()?  contextExceeded,TResult Function( bool recentlyActive)?  poisoned,TResult Function( String message)?  failed,required TResult orElse(),}) {final _that = this;
switch (_that) {
case AgentFailure_MissingApiKey() when missingApiKey != null:
return missingApiKey();case AgentFailure_UnknownProvider() when unknownProvider != null:
return unknownProvider(_that.name);case AgentFailure_InvalidUrl() when invalidUrl != null:
return invalidUrl();case AgentFailure_Busy() when busy != null:
return busy();case AgentFailure_UnknownSession() when unknownSession != null:
return unknownSession();case AgentFailure_UnknownToolCall() when unknownToolCall != null:
return unknownToolCall();case AgentFailure_RateLimited() when rateLimited != null:
return rateLimited();case AgentFailure_ContextExceeded() when contextExceeded != null:
return contextExceeded();case AgentFailure_Poisoned() when poisoned != null:
return poisoned(_that.recentlyActive);case AgentFailure_Failed() when failed != null:
return failed(_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  missingApiKey,required TResult Function( String name)  unknownProvider,required TResult Function()  invalidUrl,required TResult Function()  busy,required TResult Function()  unknownSession,required TResult Function()  unknownToolCall,required TResult Function()  rateLimited,required TResult Function()  contextExceeded,required TResult Function( bool recentlyActive)  poisoned,required TResult Function( String message)  failed,}) {final _that = this;
switch (_that) {
case AgentFailure_MissingApiKey():
return missingApiKey();case AgentFailure_UnknownProvider():
return unknownProvider(_that.name);case AgentFailure_InvalidUrl():
return invalidUrl();case AgentFailure_Busy():
return busy();case AgentFailure_UnknownSession():
return unknownSession();case AgentFailure_UnknownToolCall():
return unknownToolCall();case AgentFailure_RateLimited():
return rateLimited();case AgentFailure_ContextExceeded():
return contextExceeded();case AgentFailure_Poisoned():
return poisoned(_that.recentlyActive);case AgentFailure_Failed():
return failed(_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  missingApiKey,TResult? Function( String name)?  unknownProvider,TResult? Function()?  invalidUrl,TResult? Function()?  busy,TResult? Function()?  unknownSession,TResult? Function()?  unknownToolCall,TResult? Function()?  rateLimited,TResult? Function()?  contextExceeded,TResult? Function( bool recentlyActive)?  poisoned,TResult? Function( String message)?  failed,}) {final _that = this;
switch (_that) {
case AgentFailure_MissingApiKey() when missingApiKey != null:
return missingApiKey();case AgentFailure_UnknownProvider() when unknownProvider != null:
return unknownProvider(_that.name);case AgentFailure_InvalidUrl() when invalidUrl != null:
return invalidUrl();case AgentFailure_Busy() when busy != null:
return busy();case AgentFailure_UnknownSession() when unknownSession != null:
return unknownSession();case AgentFailure_UnknownToolCall() when unknownToolCall != null:
return unknownToolCall();case AgentFailure_RateLimited() when rateLimited != null:
return rateLimited();case AgentFailure_ContextExceeded() when contextExceeded != null:
return contextExceeded();case AgentFailure_Poisoned() when poisoned != null:
return poisoned(_that.recentlyActive);case AgentFailure_Failed() when failed != null:
return failed(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class AgentFailure_MissingApiKey extends AgentFailure {
  const AgentFailure_MissingApiKey(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_MissingApiKey);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'AgentFailure.missingApiKey()';
}


}




/// @nodoc


class AgentFailure_UnknownProvider extends AgentFailure {
  const AgentFailure_UnknownProvider({required this.name}): super._();
  

 final  String name;

/// Create a copy of AgentFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AgentFailure_UnknownProviderCopyWith<AgentFailure_UnknownProvider> get copyWith => _$AgentFailure_UnknownProviderCopyWithImpl<AgentFailure_UnknownProvider>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_UnknownProvider&&(identical(other.name, name) || other.name == name));
}


@override
int get hashCode => Object.hash(runtimeType,name);

@override
String toString() {
  return 'AgentFailure.unknownProvider(name: $name)';
}


}

/// @nodoc
abstract mixin class $AgentFailure_UnknownProviderCopyWith<$Res> implements $AgentFailureCopyWith<$Res> {
  factory $AgentFailure_UnknownProviderCopyWith(AgentFailure_UnknownProvider value, $Res Function(AgentFailure_UnknownProvider) _then) = _$AgentFailure_UnknownProviderCopyWithImpl;
@useResult
$Res call({
 String name
});




}
/// @nodoc
class _$AgentFailure_UnknownProviderCopyWithImpl<$Res>
    implements $AgentFailure_UnknownProviderCopyWith<$Res> {
  _$AgentFailure_UnknownProviderCopyWithImpl(this._self, this._then);

  final AgentFailure_UnknownProvider _self;
  final $Res Function(AgentFailure_UnknownProvider) _then;

/// Create a copy of AgentFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,}) {
  return _then(AgentFailure_UnknownProvider(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class AgentFailure_InvalidUrl extends AgentFailure {
  const AgentFailure_InvalidUrl(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_InvalidUrl);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'AgentFailure.invalidUrl()';
}


}




/// @nodoc


class AgentFailure_Busy extends AgentFailure {
  const AgentFailure_Busy(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_Busy);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'AgentFailure.busy()';
}


}




/// @nodoc


class AgentFailure_UnknownSession extends AgentFailure {
  const AgentFailure_UnknownSession(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_UnknownSession);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'AgentFailure.unknownSession()';
}


}




/// @nodoc


class AgentFailure_UnknownToolCall extends AgentFailure {
  const AgentFailure_UnknownToolCall(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_UnknownToolCall);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'AgentFailure.unknownToolCall()';
}


}




/// @nodoc


class AgentFailure_RateLimited extends AgentFailure {
  const AgentFailure_RateLimited(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_RateLimited);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'AgentFailure.rateLimited()';
}


}




/// @nodoc


class AgentFailure_ContextExceeded extends AgentFailure {
  const AgentFailure_ContextExceeded(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_ContextExceeded);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'AgentFailure.contextExceeded()';
}


}




/// @nodoc


class AgentFailure_Poisoned extends AgentFailure {
  const AgentFailure_Poisoned({required this.recentlyActive}): super._();
  

 final  bool recentlyActive;

/// Create a copy of AgentFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AgentFailure_PoisonedCopyWith<AgentFailure_Poisoned> get copyWith => _$AgentFailure_PoisonedCopyWithImpl<AgentFailure_Poisoned>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_Poisoned&&(identical(other.recentlyActive, recentlyActive) || other.recentlyActive == recentlyActive));
}


@override
int get hashCode => Object.hash(runtimeType,recentlyActive);

@override
String toString() {
  return 'AgentFailure.poisoned(recentlyActive: $recentlyActive)';
}


}

/// @nodoc
abstract mixin class $AgentFailure_PoisonedCopyWith<$Res> implements $AgentFailureCopyWith<$Res> {
  factory $AgentFailure_PoisonedCopyWith(AgentFailure_Poisoned value, $Res Function(AgentFailure_Poisoned) _then) = _$AgentFailure_PoisonedCopyWithImpl;
@useResult
$Res call({
 bool recentlyActive
});




}
/// @nodoc
class _$AgentFailure_PoisonedCopyWithImpl<$Res>
    implements $AgentFailure_PoisonedCopyWith<$Res> {
  _$AgentFailure_PoisonedCopyWithImpl(this._self, this._then);

  final AgentFailure_Poisoned _self;
  final $Res Function(AgentFailure_Poisoned) _then;

/// Create a copy of AgentFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? recentlyActive = null,}) {
  return _then(AgentFailure_Poisoned(
recentlyActive: null == recentlyActive ? _self.recentlyActive : recentlyActive // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class AgentFailure_Failed extends AgentFailure {
  const AgentFailure_Failed({required this.message}): super._();
  

 final  String message;

/// Create a copy of AgentFailure
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AgentFailure_FailedCopyWith<AgentFailure_Failed> get copyWith => _$AgentFailure_FailedCopyWithImpl<AgentFailure_Failed>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is AgentFailure_Failed&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'AgentFailure.failed(message: $message)';
}


}

/// @nodoc
abstract mixin class $AgentFailure_FailedCopyWith<$Res> implements $AgentFailureCopyWith<$Res> {
  factory $AgentFailure_FailedCopyWith(AgentFailure_Failed value, $Res Function(AgentFailure_Failed) _then) = _$AgentFailure_FailedCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$AgentFailure_FailedCopyWithImpl<$Res>
    implements $AgentFailure_FailedCopyWith<$Res> {
  _$AgentFailure_FailedCopyWithImpl(this._self, this._then);

  final AgentFailure_Failed _self;
  final $Res Function(AgentFailure_Failed) _then;

/// Create a copy of AgentFailure
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(AgentFailure_Failed(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
