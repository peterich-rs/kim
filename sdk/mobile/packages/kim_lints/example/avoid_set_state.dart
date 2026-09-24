// expect_lint: avoid_set_state
void demo(void Function(void Function()) setState) {
  setState(() {});
}
