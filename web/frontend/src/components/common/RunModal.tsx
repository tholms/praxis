import { useState, useEffect, useRef } from 'react';
import { Zap, GitBranch } from 'lucide-react';
import { Modal } from './Modal';
import { TargetSpecEditor } from './TargetSpecEditor';
import type { NodeState, TargetSpec } from '../../api/types';

export interface RunItem {
  id: string;
  name: string;
  description?: string;
  badge?: string;
}

interface RunModalProps {
  isOpen: boolean;
  onClose: () => void;
  onRun: (itemId: string, targetSpec: TargetSpec) => void;
  title: string;
  items: RunItem[];
  variant: 'operation' | 'chain';
  preSelectedItem?: RunItem | null;
  nodes?: NodeState[];
  warningMessage?: string;
  //
  // Pre-fill the TargetSpec when the modal opens (e.g., from NodeCard
  // to pre-select the current node).
  //
  initialTargetSpec?: TargetSpec;
}

const EMPTY_TARGET_SPEC: TargetSpec = {
  node_ids: [],
  os_filter: null,
  agent_short_names: [],
  include_triggering_node: false,
};

export function RunModal({
  isOpen,
  onClose,
  onRun,
  title,
  items,
  variant,
  preSelectedItem,
  nodes = [],
  warningMessage,
  initialTargetSpec,
}: RunModalProps) {
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [targetSpec, setTargetSpec] = useState<TargetSpec>(initialTargetSpec ?? EMPTY_TARGET_SPEC);

  const Icon = variant === 'operation' ? Zap : GitBranch;
  const isOperation = variant === 'operation';
  const prevIsOpen = useRef(false);

  //
  // Reset state when modal opens.
  //

  useEffect(() => {
    const justOpened = isOpen && !prevIsOpen.current;
    prevIsOpen.current = isOpen;

    if (justOpened) {
      setSelectedItemId(preSelectedItem?.id ?? null);
      setTargetSpec(initialTargetSpec ?? EMPTY_TARGET_SPEC);
    }
  }, [isOpen, preSelectedItem, initialTargetSpec]);

  const handleRun = () => {
    if (!selectedItemId) return;
    onRun(selectedItemId, targetSpec);
    onClose();
  };

  const isSingleSelect = !!preSelectedItem;

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={title} size="sm">
      <div className="space-y-0">
        {warningMessage && (
          <div className="p-2 bg-[var(--bg-secondary)]">
            <div className="flex items-start gap-1.5 p-2 bg-[var(--accent-warning)]/10 border border-[var(--accent-warning)]/30">
              <span className="text-[var(--accent-warning)] text-xs mt-px">⚠</span>
              <p className="text-[10px] text-[var(--accent-warning)]">{warningMessage}</p>
            </div>
          </div>
        )}

        {/*
        //
        // Item selection section.
        //
        */}
        <div className="p-2 bg-[var(--bg-secondary)]">
          {isSingleSelect && preSelectedItem ? (
            <div className={`px-2.5 py-2 border ${isOperation ? 'bg-[var(--accent-purple)]/10 border-[var(--accent-purple)]/30' : 'bg-[var(--accent-info)]/10 border-[var(--accent-info)]/30'}`}>
              <div className="flex items-center justify-between">
                <span className="font-medium text-xs text-highlight">{preSelectedItem.name}</span>
                {preSelectedItem.badge && (
                  <span className="text-[10px]" style={{ color: 'var(--text-muted)' }}>{preSelectedItem.badge}</span>
                )}
              </div>
              {preSelectedItem.description && (
                <p className="text-[10px] mt-0.5" style={{ color: 'var(--text-muted)' }}>{preSelectedItem.description}</p>
              )}
            </div>
          ) : (
            <>
              {items.length === 0 ? (
                <div className="p-4 text-center">
                  <Icon size={20} className="mx-auto mb-2 text-muted opacity-50" />
                  <p className="text-muted text-[10px]">No {variant} definitions available</p>
                  <p className="text-[9px] mt-0.5" style={{ color: 'var(--text-muted)' }}>Add {variant === 'operation' ? 'operations' : 'chains'} in the Operations page</p>
                </div>
              ) : (
                <div className="space-y-1 max-h-40 overflow-y-auto scrollbar-on-hover">
                  {items.map((item) => (
                    <div
                      key={item.id}
                      onClick={() => setSelectedItemId(item.id)}
                      className={`px-2.5 py-1.5 cursor-pointer transition-colors border ${
                        selectedItemId === item.id
                          ? isOperation
                            ? 'bg-[var(--accent-purple)]/20 border-[var(--accent-purple)]'
                            : 'bg-[var(--accent-info)]/20 border-[var(--accent-info)]'
                          : 'bg-[var(--bg-primary)] border-dim hover:border-subtle'
                      }`}
                    >
                      <div className="flex items-center justify-between">
                        <span className="font-medium text-xs text-highlight">{item.name}</span>
                        {item.badge && (
                          <span className="text-[10px]" style={{ color: 'var(--text-muted)' }}>{item.badge}</span>
                        )}
                      </div>
                      {item.description && (
                        <p className="text-[10px] mt-0.5 line-clamp-2" style={{ color: 'var(--text-muted)' }}>{item.description}</p>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </div>

        {/*
        //
        // Target spec section.
        //
        */}
        <div className="p-2 bg-[var(--bg-secondary)]">
          <div className="text-[9px] tracking-widest text-[var(--text-secondary)] mb-1.5" style={{ letterSpacing: '0.08em' }}>
            TARGET
          </div>
          <TargetSpecEditor
            value={targetSpec}
            onChange={setTargetSpec}
            nodes={nodes}
          />
        </div>

        {/*
        //
        // Actions.
        //
        */}
        <div className="p-2 bg-[var(--bg-secondary)]">
          <div className="flex justify-end gap-1.5">
            <button
              onClick={onClose}
              className="px-3 py-1.5 text-[10px] tracking-wider text-muted border border-dim hover:border-subtle hover:bg-[var(--highlight)] transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleRun}
              disabled={!selectedItemId}
              className={`inline-flex items-center gap-1.5 px-3 py-1.5 text-[10px] tracking-wider border border-dim transition-colors disabled:opacity-50 ${
                isOperation
                  ? 'bg-[var(--accent-purple)]/20 text-[var(--accent-purple)] hover:border-[var(--accent-purple)] hover:bg-[var(--accent-purple)]/30'
                  : 'bg-[var(--accent-info)]/20 text-[var(--accent-info)] hover:border-[var(--accent-info)] hover:bg-[var(--accent-info)]/30'
              }`}
            >
              <Icon size={11} />
              Run
            </button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
