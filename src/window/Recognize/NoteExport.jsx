import React, { useState } from 'react';
import { NotebookPen } from 'lucide-react';
import { Button, Input, Modal, ModalBody, ModalContent, ModalFooter, ModalHeader } from '@nextui-org/react';
import { invoke } from '@tauri-apps/api/tauri';
import { writeText } from '@tauri-apps/api/clipboard';
import { useAtomValue } from 'jotai';
import { useTranslation } from 'react-i18next';
import { translateOnce } from '../../utils/translate_once';
import { base64Atom } from './ImageArea';
import { textAtom } from './TextArea';

function noteMarkdown(note) {
    const fields = [
        [note.textLabel, note.text],
        [note.translationLabel, note.translation],
        [note.sourceLabel, note.source],
    ];
    return [
        `# ${note.title}`,
        ...fields
            .filter(([, value]) => value.trim())
            .map(([label, value]) => {
                const fenceLength = (value.match(/`+/g) || []).reduce(
                    (length, run) => Math.max(length, run.length + 1),
                    3
                );
                const fence = '`'.repeat(fenceLength);
                return `## ${label}\n\n${fence}\n${value}\n${fence}`;
            }),
    ].join('\n\n');
}

function NoteDialog({ snapshot, close }) {
    const { t } = useTranslation();
    const [source, setSource] = useState('');
    const [translation, setTranslation] = useState('');
    const [busy, setBusy] = useState('');
    const [error, setError] = useState('');
    const [status, setStatus] = useState('');
    const note = {
        ...snapshot,
        title: t('recognize.note_title'),
        imageLabel: t('recognize.original_image'),
        textLabel: t('recognize.result'),
        translationLabel: t('recognize.note_translation'),
        sourceLabel: t('recognize.note_source'),
        translation,
        source: source.trim(),
    };

    const run = async (action) => {
        setBusy(action);
        setError('');
        setStatus('');
        try {
            if (action === 'translate') {
                setTranslation(await translateOnce(note.text));
            } else if (action === 'export') {
                if (await invoke('save_screenshot_note', { note })) setStatus(t('recognize.note_saved'));
            } else {
                await writeText(noteMarkdown(note));
                setStatus(t('recognize.note_copied'));
            }
        } catch (error) {
            setError(t(error?.message ?? String(error)));
        } finally {
            setBusy('');
        }
    };

    return (
        <Modal
            isOpen
            onClose={close}
            scrollBehavior='inside'
            isDismissable={!busy}
            isKeyboardDismissDisabled={!!busy}
            hideCloseButton={!!busy}
        >
            <ModalContent className='max-h-[90vh]'>
                <ModalHeader>{note.title}</ModalHeader>
                <ModalBody>
                    <Input
                        label={note.sourceLabel}
                        value={source}
                        onValueChange={setSource}
                    />
                    {note.text && (
                        <section>
                            <h3 className='text-small font-semibold'>{note.textLabel}</h3>
                            <p className='max-h-24 overflow-auto whitespace-pre-wrap break-words text-small'>
                                {note.text}
                            </p>
                        </section>
                    )}
                    {translation && (
                        <section>
                            <h3 className='text-small font-semibold'>{note.translationLabel}</h3>
                            <p className='max-h-24 overflow-auto whitespace-pre-wrap break-words text-small'>
                                {translation}
                            </p>
                        </section>
                    )}
                    {error && (
                        <p
                            role='alert'
                            className='text-small text-danger'
                        >
                            {error}
                        </p>
                    )}
                    {status && (
                        <p
                            role='status'
                            className='text-small'
                        >
                            {status}
                        </p>
                    )}
                </ModalBody>
                <ModalFooter className='flex-wrap'>
                    <Button
                        size='sm'
                        variant='light'
                        isDisabled={!!busy || !note.text.trim()}
                        isLoading={busy === 'translate'}
                        onPress={() => run('translate')}
                    >
                        {t('recognize.note_translate')}
                    </Button>
                    <Button
                        size='sm'
                        variant='flat'
                        isDisabled={!!busy || (!note.text.trim() && !translation && !note.source)}
                        isLoading={busy === 'copy'}
                        onPress={() => run('copy')}
                    >
                        {t('recognize.note_copy_markdown')}
                    </Button>
                    <Button
                        size='sm'
                        color='primary'
                        isDisabled={!!busy}
                        isLoading={busy === 'export'}
                        onPress={() => run('export')}
                    >
                        {t('recognize.note_export_html')}
                    </Button>
                </ModalFooter>
            </ModalContent>
        </Modal>
    );
}

export default function NoteExport({ onOpenChange }) {
    const { t } = useTranslation();
    const image = useAtomValue(base64Atom);
    const text = useAtomValue(textAtom);
    const [snapshot, setSnapshot] = useState(null);

    return (
        <>
            <Button
                size='sm'
                variant='flat'
                startContent={<NotebookPen size={16} />}
                isDisabled={!image}
                onPress={() => {
                    setSnapshot({ image, text: text ?? '' });
                    onOpenChange(true);
                }}
            >
                {t('recognize.note_title')}
            </Button>
            {snapshot && (
                <NoteDialog
                    snapshot={snapshot}
                    close={() => {
                        setSnapshot(null);
                        onOpenChange(false);
                    }}
                />
            )}
        </>
    );
}
