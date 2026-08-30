/// User-visible Chinese copy. Matches H5 `sdk/web/app/copy.ts`.
library;

abstract final class Copy {
  static const brand = 'KIM';
  static const brandSub = '即时通讯';

  static const loginTitle = '登录';
  static const registerTitle = '创建账号';
  static const account = '账号';
  static const password = '密码';
  static const confirmPassword = '确认密码';
  static const accountPlaceholder = '请输入账号';
  static const passwordPlaceholder = '请输入密码';
  static const confirmPlaceholder = '再次输入密码';
  static const accountHint = '3–32 位字母、数字或下划线';
  static const passwordHint = '8–128 位';
  static const loginAction = '登录';
  static const registerAction = '注册';
  static const submittingLogin = '登录中…';
  static const submittingRegister = '注册中…';
  static const noAccount = '没有账号？';
  static const goRegister = '注册';
  static const hasAccount = '已有账号？';
  static const goLogin = '登录';
  static const mismatch = '两次输入的密码不一致';
  static const invalidAccount = '账号需为 3–32 位字母、数字或下划线';
  static const invalidPassword = '密码需为 8–128 位';
  static const badCredentials = '账号或密码错误';
  static const accountExists = '账号已存在';
  static const network = '网络异常，请稍后重试';
  static const unavailable = '服务暂时不可用，请稍后重试';
  static const authFailed = '登录失败，请稍后重试';
  static const timeout = '连接超时，请稍后重试';
  static const logout = '退出登录';
  static const loggingOut = '退出中…';
  static const changePassword = '修改密码';
  static const oldPassword = '当前密码';
  static const newPassword = '新密码';
  static const passwordChanged = '密码已更新';
  static const save = '保存';
  static const localServer = '本地 :8080';
  static const prodServer = '生产';
  static const signedInAs = '已登录';
}
