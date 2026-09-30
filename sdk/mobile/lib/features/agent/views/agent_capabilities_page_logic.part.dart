part of 'agent_capabilities_page.dart';

// ignore: library_private_types_in_public_api
extension AgentCapabilitiesPageLogic on _AgentCapabilitiesPageState {
  Future<void> _toggleApp(CatalogSkill skill, bool enable) async {
    final c = _controller();
    final profile = _draft.profile;
    if (profile == null) {
      return;
    }
    if (enable) {
      final decision = await confirmAppSkillEnable(
        context: context,
        profile: profile,
        caps: _draft.caps,
        assigned: _draft.assigned,
        skill: skill,
      );
      if (decision == null || !mounted) {
        return;
      }
      await c.persist(
        caps: decision.caps,
        skills: decision.skills,
        permissionOverrides: Map<String, String>.from(
          decision.permissionOverrides,
        ),
      );
      return;
    }
    final skills = [
      for (final s in _draft.assigned)
        if (s.id != skill.id) s,
    ];
    await c.persist(skills: skills);
  }

  Future<void> _toggleDenylist(String id, bool muted) async {
    final next = List<String>.from(_draft.denylist);
    if (muted) {
      if (!next.contains(id)) {
        next.add(id);
      }
    } else {
      next.remove(id);
    }
    await _controller().persist(denylist: next);
  }
}
