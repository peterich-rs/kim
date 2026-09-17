import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/pet_pack.dart';

void main() {
  test('default grid is 8x9 / 192x208', () {
    final pack = PetPack.parseJson(<dynamic, dynamic>{}, Uint8List(0));
    expect(pack.columns, 8);
    expect(pack.rows, 9);
    expect(pack.frameWidth, 192);
    expect(pack.frameHeight, 208);
  });

  test('waving maps to PetPhase.wave', () {
    final pack = PetPack.parseJson(<dynamic, dynamic>{
      'animations': {
        'idle': {'row': 0, 'frames': 6, 'fps': 8},
        'waving': {'row': 3, 'frames': 4, 'fps': 8},
      },
    }, Uint8List(0));
    expect(pack.clipFor(PetPhase.wave).row, 3);
    expect(pack.clips[PetPhase.wave]?.row, 3);
  });

  test('missing review falls back to idle clip', () {
    final pack = PetPack.parseJson(<dynamic, dynamic>{
      'animations': {
        'idle': {'row': 0, 'frames': 6, 'fps': 8},
        'waving': {'row': 3, 'frames': 4, 'fps': 8},
        'failed': {'row': 5, 'frames': 8, 'fps': 8},
        'running': {'row': 7, 'frames': 6, 'fps': 8},
      },
    }, Uint8List(0));
    expect(pack.clipFor(PetPhase.review).row, pack.clipFor(PetPhase.idle).row);
    expect(
      pack.clipFor(PetPhase.review).frames,
      pack.clipFor(PetPhase.idle).frames,
    );
  });

  test('srcRect(running, 0) top is 7 * 208', () {
    final pack = PetPack.parseJson(<dynamic, dynamic>{}, Uint8List(0));
    expect(pack.srcRect(PetPhase.running, 0).top, 7 * 208);
  });

  test('frame index wraps with % frames', () {
    final pack = PetPack.parseJson(<dynamic, dynamic>{}, Uint8List(0));
    final frames = pack.clipFor(PetPhase.idle).frames;
    expect(frames, 6);
    expect(
      pack.srcRect(PetPhase.idle, frames).left,
      pack.srcRect(PetPhase.idle, 0).left,
    );
    expect(
      pack.srcRect(PetPhase.idle, frames + 1).left,
      pack.srcRect(PetPhase.idle, 1).left,
    );
  });
}
