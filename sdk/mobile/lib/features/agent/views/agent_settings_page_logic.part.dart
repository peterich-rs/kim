part of 'agent_settings_page.dart';

// ignore: library_private_types_in_public_api

Future<void> _reloadSurfaceOf(
  _AgentEditorPageState state, {
  required bool toastDropped,
}) async {
  final account = state._selectedAccount;
  final vendorId = account?.vendorId ?? '';
  if (vendorId.isEmpty) {
    return;
  }
  try {
    final catalog = state.ref.read(catalogRepositoryProvider);
    final surface = await catalog.surface(
      vendor: vendorId,
      model: state._model.text.trim(),
    );
    if (!state.mounted) {
      return;
    }
    final aligned = alignChoice(surface, state._choice);
    state._form.setSurface(surface: surface, choice: aligned.choice);
    if (toastDropped && aligned.dropped) {
      state._toastInfo(Copy.agentReasoningDropped);
    }
  } catch (_) {}
}

// ignore: library_private_types_in_public_api
extension AgentEditorPageLogic on _AgentEditorPageState {
  Future<void> _reloadSurface({required bool toastDropped}) =>
      _reloadSurfaceOf(this, toastDropped: toastDropped);

  Future<void> _openNewProvider() async {
    final id = await openProviderAccountEditor(context);
    if (!mounted || id == null || id.isEmpty) {
      return;
    }
    _selectAccount(id);
  }

  void _selectAccount(String id) {
    if (id == _kNewProvider) {
      unawaited(_openNewProvider());
      return;
    }
    final account = ref.read(providerAccountsProvider.notifier).byId(id);
    if (account == null) {
      return;
    }
    var model = _model.text.trim();
    var fell = false;
    if (account.models.isNotEmpty && !account.models.contains(model)) {
      model = defaultModelForAccount(account, _vendorById(account.vendorId));
      fell = true;
    }
    _model.text = model;
    _form.bindAccount(id: id, contextTokens: defaultContextTokens(model));
    if (fell && mounted) {
      _toastInfo(AppLocalizations.of(context).agentModelFallback(model));
    }
    unawaited(_reloadSurface(toastDropped: true));
  }

  Future<void> _pickModel() async {
    final models = _modelOptions;
    if (!mounted) {
      return;
    }
    final picked = await showModelPickerSheet(
      context: context,
      models: models,
      selected: _model.text.trim(),
    );
    if (picked == null || !mounted) {
      return;
    }
    if (isOtherModelSentinel(picked)) {
      await _otherModel();
      return;
    }
    _model.text = picked;
    _form.setContextTokens(defaultContextTokens(picked));
    unawaited(_reloadSurface(toastDropped: true));
  }

  Future<void> _otherModel() async {
    final raw = await showOtherModelDialog(
      context: context,
      hint: Copy.agentModel,
    );
    if (raw == null || !isSelectableModelId(raw) || !mounted) {
      return;
    }
    final account = _selectedAccount;
    if (account != null) {
      final models = selectableModelIds([...account.models, raw]);
      await ref
          .read(providerAccountsProvider.notifier)
          .upsert(account.copyWith(models: models));
      _model.text = raw;
      _form.setContextTokens(defaultContextTokens(raw));
    } else {
      _model.text = raw;
      _form.rememberCustomModel(
        pendingModels: selectableModelIds([..._pendingModels, raw]),
        contextTokens: defaultContextTokens(raw),
      );
    }
    unawaited(_reloadSurface(toastDropped: true));
  }

  void _openChat(AgentProfile profile) {
    final person = personForProfile(profile);
    openKimChat(
      context,
      ref,
      id: person.account,
      kind: ThreadKind.user,
      title: profile.displayName,
    );
  }

  List<DropdownMenuItem<String>> _providerItems(
    AppLocalizations l10n,
    AgentSettingsDraft settings,
  ) {
    final accounts = ref.watch(providerAccountsProvider);
    return <DropdownMenuItem<String>>[
      for (final a in accounts)
        DropdownMenuItem(
          value: a.id,
          child: Text(
            a.displayName.isNotEmpty
                ? a.displayName
                : (_vendorById(a.vendorId, settings.vendors)?.displayName ??
                      a.vendorId),
          ),
        ),
      DropdownMenuItem(
        value: _kNewProvider,
        child: Text(l10n.agentNewProvider),
      ),
    ];
  }
}
