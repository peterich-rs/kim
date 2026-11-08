/// Stable names for the generated FFI data types the UI renders.
///
/// This is the only handwritten module that re-exports generated projection
/// data. Capability calls stay on the ports; these types carry no behavior.
/// `ThreadKind` is hidden because `models/thread.dart` owns the UI spelling.
library;

export 'package:kim_mobile/src/rust/api/types.dart';
