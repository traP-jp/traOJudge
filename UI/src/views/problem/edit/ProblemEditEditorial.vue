<script setup lang="ts">
import { useRoute } from 'vue-router'
import { computed, onMounted, ref, watch } from 'vue'
import PlainTextArea from '@/components/PlainTextArea.vue'
import AlertBox from '@/components/AlertBox.vue'
import PrimaryButton from '@/components/Controls/PrimaryButton.vue'
import PlainTextbox from '@/components/Controls/Textbox/PlainTextbox.vue'
import MaterialIcon from '@/components/MaterialIcon.vue'
import TextLink from '@/components/TextLink.vue'
import { EditorialsApi } from '@/api/generated'
import { apiErrorMessage, requireEtag } from '@/utils/apiError'
import { useMockAuthGuard } from '@/composables/useMockAuthGuard'
import router from '@/router'

const { requireMockLogin } = useMockAuthGuard()

const title = ref<string>('')
const body = ref<string>('')
const etag = ref('')

const titleError = ref<string>('')

const saveError = ref<string>('')
const saveSuccess = ref<boolean>(false)
const saveErrorShow = ref<boolean>(false)

const isLoading = ref<boolean>(false)
const inputInvalid = computed<boolean>(() => {
  return !title.value || !body.value || !!titleError.value
})

const route = useRoute()

const editorialApi = new EditorialsApi()

if (typeof route.params.id !== 'string') throw new Error('Invalid route')

const problemId = ref<string>('')
const editorialId = ref<string>('')

watch(
  () => route.params.id,
  (id) => {
    problemId.value = `${id}`
  },
  { immediate: true }
)

watch(
  () => route.params.editorialId,
  (id) => {
    editorialId.value = id as string
  },
  { immediate: true }
)

function onChangeTitle() {
  title.value = title.value.trim()
}

function onChangeBody() {
  body.value = body.value.trim()
}

async function fetchEditorial() {
  if (editorialId.value === 'new') return

  isLoading.value = true

  try {
    const result = await editorialApi.getEditorialRaw({
      editorialId: editorialId.value
    })

    etag.value = requireEtag(result.raw)
    const response = await result.value()
    title.value = response.title
    body.value = response.body
  } catch (error) {
    console.error('解説取得エラー:', error)

    saveError.value = await apiErrorMessage(error)

    saveErrorShow.value = true
  } finally {
    isLoading.value = false
  }
}

async function saveEditorial() {
  saveError.value = ''
  saveSuccess.value = false
  saveErrorShow.value = false

  isLoading.value = true

  try {
    if (editorialId.value === 'new') {
      const response = await requireMockLogin(() =>
        editorialApi.createEditorial({
          origin: window.location.origin,
          idempotencyKey: crypto.randomUUID(),
          editorialCreate: { problemId: problemId.value, title: title.value, body: body.value }
        })
      )
      await router.push(`/problems/${problemId.value}/edit/editorials/${response.id}`)
      await fetchEditorial()
    } else {
      const response = await requireMockLogin(() =>
        editorialApi.updateEditorialRaw({
          origin: window.location.origin,
          ifMatch: etag.value,
          editorialId: editorialId.value,
          editorialUpdate: { title: title.value, body: body.value }
        })
      )
      etag.value = requireEtag(response.raw)
    }
    saveSuccess.value = true
  } catch (error) {
    saveError.value = await apiErrorMessage(error)
    saveErrorShow.value = true
  } finally {
    isLoading.value = false
  }
}

onMounted(() => {
  fetchEditorial()
})
</script>

<template>
  <div class="flex font-primary">
    <div class="flex w-full flex-col">
      <div class="flex w-full flex-col items-start justify-start gap-4">
        <TextLink href="../editorials">
          <span class="inline-flex items-center gap-1">
            <MaterialIcon icon="arrow_left_alt" class="inline-block" />
            <span class="underline underline-offset-2">解説一覧に戻る</span>
          </span>
        </TextLink>

        <h2 class="text-xl font-medium">解説の編集</h2>

        <div class="w-full max-w-profile-max">
          <PlainTextbox
            id="display-name"
            v-model="title"
            class="h-5.5 w-full"
            label="解説タイトル"
            required
            displays-length
            :error-message="titleError"
            @change="onChangeTitle"
          />
        </div>

        <div class="w-full">
          <PlainTextArea
            id="body"
            v-model="body"
            class="h-64 w-full"
            label="本文"
            required
            @change="onChangeBody"
          />
        </div>

        <PrimaryButton
          class="h-10 w-18 px-3 py-2"
          :disabled="
            isLoading ||
            inputInvalid ||
            title.length > 200 ||
            body.length > 200000 ||
            (editorialId !== 'new' && !etag)
          "
          @click="saveEditorial"
        >
          保存
        </PrimaryButton>
        <AlertBox v-model:show="saveSuccess" text="解説が保存されました．" />
        <AlertBox v-model:show="saveErrorShow" :text="saveError" type="error" />
      </div>
    </div>
  </div>
</template>
