import { watch } from 'vue'
import { createRouter, createWebHistory } from 'vue-router'

import { useMockAuthGuard } from '@/composables/problems/useMockAuthGuard'
import { useUserStore } from '@/stores/user'
import TopView from '@/views/TopView.vue'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- ESLintでは、.vueからimportしたコンポーネントの型を解決できないため。
      component: TopView,
      meta: { requiresAuth: true }
    },
    { path: '/signup/:pathMatch(.*)*', component: () => import('@/views/AuthTodoView.vue') },
    { path: '/login', component: () => import('@/views/LoginView.vue') },
    {
      path: '/contest/:contestId',
      name: 'contest-top',
      component: () => import('@/views/ContestView.vue')
    },
    {
      path: '/contest/:contestId/editorials',
      name: 'contest-editorials',
      component: () => import('@/views/ContestEditorialsView.vue')
    },
    {
      path: '/contest/:contestId/problems/:problemId/editorials/:editorialId',
      name: 'contest-editorial',
      component: () => import('@/views/ContestEditorialView.vue')
    },
    {
      path: '/reset-password/:pathMatch(.*)*',
      component: () => import('@/views/AuthTodoView.vue')
    },
    { path: '/oauth/:pathMatch(.*)*', component: () => import('@/views/AuthTodoView.vue') },
    {
      path: '/users/:id',
      component: () => import('@/views/UserView.vue'),
      meta: { requiresAuth: true },
      children: [
        {
          path: '',
          name: 'user-home',
          component: () => import('@/views/user/UserHome.vue')
        },
        {
          path: 'submissions',
          component: () => import('@/views/user/UserSubmissions.vue')
        },
        { path: 'problems', component: () => import('@/views/user/UserProblems.vue') }
      ]
    },
    {
      path: '/problems',
      component: () => import('@/views/ProblemsView.vue'),
      meta: { requiresAuth: true }
    },
    {
      path: '/problems/create',
      component: () => import('@/views/problem/create/ProblemCreateView.vue'),
      meta: { requiresAuth: true, requiresMockAuth: true }
    },
    {
      path: '/problems/:id',
      component: () => import('@/views/ProblemView.vue'),
      meta: { requiresAuth: true },
      children: [
        {
          path: '',
          name: 'problem-home',
          component: () => import('@/views/problem/ProblemHome.vue')
        },
        {
          path: 'submit',
          name: 'problem-submit',
          component: () => import('@/views/problem/ProblemSubmit.vue')
        },
        {
          path: 'submissions',
          name: 'problem-submissions',
          component: () => import('@/views/problem/ProblemSubmissions.vue')
        },
        {
          path: 'submissions/:submissionId',
          component: () => import('@/views/problem/ProblemSubmission.vue')
        },
        {
          path: 'editorials',
          name: 'problem-editorials',
          component: () => import('@/views/problem/ProblemEditorials.vue')
        },
        {
          path: 'editorials/:editorialId',
          component: () => import('@/views/problem/ProblemEditorial.vue')
        }
      ]
    },
    {
      path: '/problems/:id/edit',
      component: () => import('@/views/ProblemEditView.vue'),
      meta: { requiresAuth: true, requiresMockAuth: true },
      children: [
        {
          path: '',
          redirect: (to) => {
            const id = to.params.id
            if (typeof id !== 'string') throw new Error('Invalid route')
            return `/problems/${id}/edit/basic`
          }
        },
        {
          path: 'basic',
          component: () => import('@/views/problem/edit/ProblemEditBasic.vue')
        },
        {
          path: 'statement',
          component: () => import('@/views/problem/edit/ProblemEditStatement.vue')
        },
        {
          path: 'testcases',
          component: () => import('@/views/problem/edit/ProblemEditTestcases.vue')
        },
        {
          path: 'editorials',
          component: () => import('@/views/problem/edit/ProblemEditEditorials.vue')
        },
        {
          path: 'editorials/:editorialId',
          component: () => import('@/views/problem/edit/ProblemEditEditorial.vue')
        }
      ]
    },
    {
      path: '/settings',
      component: () => import('@/views/SettingsView.vue'),
      meta: { requiresAuth: true },
      children: [
        { path: '', redirect: '/settings/account' },
        {
          path: 'account',
          component: () => import('@/views/settings/SettingsAccount.vue')
        },
        {
          path: 'profile',
          component: () => import('@/views/settings/SettingsProfile.vue')
        }
      ]
    }
  ]
})

router.beforeEach(async (to) => {
  const userStore = useUserStore()

  // 公開ページは認証APIの応答を待たず表示する。
  if (!to.meta.requiresAuth && to.path !== '/login') {
    void userStore.initialize()
    return
  }
  await userStore.initialize()
  // 通信失敗時は遷移先を保ち、Appの再試行UIを表示する。
  if (!userStore.isInitialized) return

  if (to.meta.requiresAuth && !userStore.isAuthenticated) {
    return {
      path: '/login',
      query: { redirect: to.fullPath }
    }
  }

  const { canEditProblem } = useMockAuthGuard()
  if (to.meta.requiresMockAuth && !canEditProblem.value) return '/'

  if (to.path === '/login' && userStore.isAuthenticated) return '/'
})

// セッション失効時もガードを通し、ログイン画面への遷移先を一元化する。
export const watchSessionRouting = () => {
  const session = useUserStore()
  return watch(
    () => [session.isInitialized, session.isAuthenticated],
    ([ready, authenticated]) => {
      if (!ready || authenticated || !router.currentRoute.value.meta.requiresAuth) return
      const { path, query, hash } = router.currentRoute.value
      void router.replace({ path, query, hash, force: true })
    }
  )
}

export default router
