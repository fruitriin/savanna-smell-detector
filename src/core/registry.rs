use super::{SmellDetector, SmellType, TestFile, TestSmell};
use std::collections::HashSet;

/// Detector の登録・管理
pub struct SmellDetectorRegistry {
    detectors: Vec<Box<dyn SmellDetector>>,
    enabled: Option<HashSet<SmellType>>,
    disabled: HashSet<SmellType>,
}

impl SmellDetectorRegistry {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            enabled: None,
            disabled: HashSet::new(),
        }
    }

    pub fn register(mut self, detector: impl SmellDetector + 'static) -> Self {
        self.detectors.push(Box::new(detector));
        self
    }

    pub fn disable(mut self, smell: SmellType) -> Self {
        self.disabled.insert(smell);
        self
    }

    pub fn enable_only(mut self, smells: HashSet<SmellType>) -> Self {
        self.enabled = Some(smells);
        self
    }

    /// 全ファイルに対して検出を実行
    pub fn detect_all(&self, files: &[TestFile]) -> Vec<TestSmell> {
        let mut results = Vec::new();
        for file in files {
            for detector in &self.detectors {
                let smells = detector.detect(file);
                for smell in smells {
                    if self.disabled.contains(&smell.smell_type) {
                        continue;
                    }
                    if let Some(ref enabled) = self.enabled {
                        if !enabled.contains(&smell.smell_type) {
                            continue;
                        }
                    }
                    results.push(smell);
                }
            }
        }
        results
    }
}
