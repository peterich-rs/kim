library;

import 'package:flutter_riverpod/experimental/mutation.dart';

import '../models/models.dart';

final signInMutation = Mutation<void>(label: 'auth.signIn');
final registerMutation = Mutation<void>(label: 'auth.register');
final signOutMutation = Mutation<void>(label: 'auth.signOut');
final changePasswordMutation = Mutation<void>(label: 'auth.changePassword');
final sendMessageMutation = Mutation<KimChatMsg>(label: 'inbox.send');
final friendRequestMutation = Mutation<void>(label: 'contacts.request');
final friendAcceptMutation = Mutation<void>(label: 'contacts.accept');
final friendRejectMutation = Mutation<void>(label: 'contacts.reject');
final searchPeopleMutation = Mutation<void>(label: 'contacts.search');
