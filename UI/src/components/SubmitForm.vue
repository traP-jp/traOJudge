<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import CodeBlock from '@/components/CodeBlock.vue'
import PrimaryButton from '@/components/Controls/PrimaryButton.vue'
import SelectBox, { type SelectOption } from '@/components/Controls/SelectBox.vue'
import PlainTextbox from '@/components/Controls/Textbox/PlainTextbox.vue'
import { ProblemsApi, SubmissionsApi, type Language } from '@/api/generated'
import { apiErrorMessage } from '@/utils/apiError'
import { sourceFileManifest, sourceFilePathError } from '@/utils/sourceFileManifest'

const router = useRouter()
const { problemId } = defineProps<{ problemId: string }>()
const api = new SubmissionsApi()
const languages = ref<Language[]>([])
const selectedOption = ref<SelectOption>()
const selectedLanguage = computed(() =>
  languages.value.find((item) => item.id === selectedOption.value?.id)
)
const sourceCode = ref<string | null>(null)
const filename = ref('main.cpp')
const filenameError = computed(() => sourceFilePathError(filename.value))
const isLoading = ref(true)
const isSubmitting = ref(false)
const error = ref('')

onMounted(async () => {
  try {
    languages.value = (await new ProblemsApi().listLanguages()).items
    selectedOption.value = languages.value[0]
  } catch (cause) {
    error.value = await apiErrorMessage(cause)
  } finally {
    isLoading.value = false
  }
})

async function handleSubmit() {
  if (
    isSubmitting.value ||
    !selectedLanguage.value ||
    !sourceCode.value?.trim() ||
    filenameError.value
  )
    return
  isSubmitting.value = true
  error.value = ''
  try {
    const content = new TextEncoder().encode(sourceCode.value)
    const file = await sourceFileManifest(filename.value, content)
    const upload = await api.createSubmissionUpload({
      origin: window.location.origin,
      idempotencyKey: crypto.randomUUID(),
      sourceUploadRequest: {
        problemId,
        languageId: selectedLanguage.value.id,
        context: { kind: 'practice' },
        files: [file]
      }
    })
    if (upload.targets.length !== 1 || upload.targets[0].path !== file.path) {
      throw new Error('提出ファイルの転送先を確認できませんでした．')
    }
    const target = upload.targets[0]
    const response = await fetch(target.url, {
      method: target.method,
      headers: target.headers,
      body: content,
      credentials: 'omit'
    })
    if (!response.ok) throw new Error('提出ファイルを転送できませんでした．')
    const accepted = await api.createSubmission({
      origin: window.location.origin,
      idempotencyKey: crypto.randomUUID(),
      submissionCreate: { uploadId: upload.uploadId }
    })
    await router.push(`/problems/${problemId}/submissions/${accepted.submissionId}`)
  } catch (cause) {
    error.value = await apiErrorMessage(cause)
  } finally {
    isSubmitting.value = false
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <p v-if="error" class="text-status-error" role="alert">{{ error }}</p>
    <p v-if="isLoading">読み込み中...</p>
    <template v-else>
      <h2 class="fontstyle-ui-subtitle text-text-primary">提出</h2>
      <SelectBox
        v-model="selectedOption"
        :options="languages"
        label="言語"
        required
        :disabled="isSubmitting"
      />
      <p v-if="selectedLanguage" class="text-text-secondary">{{ selectedLanguage.versionLabel }}</p>
      <PlainTextbox
        id="source-filename"
        v-model="filename"
        label="ファイル名"
        required
        supporting-text="例：main.cpp，src/main.cpp"
        :disabled="isSubmitting"
        :error-message="filenameError"
        :aria-invalid="Boolean(filenameError)"
      />
      <CodeBlock
        v-if="selectedLanguage"
        v-model="sourceCode"
        :language="selectedLanguage"
        class="w-full"
      />
      <PrimaryButton
        :disabled="
          isSubmitting || !selectedLanguage || !sourceCode?.trim() || Boolean(filenameError)
        "
        class="h-12 w-24"
        right-icon="send"
        @click="handleSubmit"
      >
        {{ isSubmitting ? '提出中...' : '提出' }}
      </PrimaryButton>
    </template>
  </div>
</template>
