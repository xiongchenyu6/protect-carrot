import tempfile
import unittest
from pathlib import Path
from stage_web_assets import stage

class StageTests(unittest.TestCase):
    def test_only_runtime_effects_and_compressed_music_are_published(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            files={'src/effects.rs':'let clip = "fire/eruption";', 'assets/pixelfx/fire/eruption.webp':'effect', 'assets/pixelfx/fire/unused.webp':'unused', 'assets/pixelfx/catalog.json':'metadata', 'assets/audio/bgm.wav':'huge', 'assets/audio/bgm.ogg':'compressed', 'assets/audio/click.wav':'click', 'assets/fonts/embedded.ttc':'embedded', 'assets/sprites/carrot.webp':'menu'}
            for name, content in files.items():
                p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(content)
            stage(root)
            actual={p.relative_to(root/'web/assets').as_posix() for p in (root/'web/assets').rglob('*') if p.is_file()}
            self.assertEqual(actual,{'pixelfx/fire/eruption.webp','pixelfx/catalog.json','audio/bgm.ogg','audio/click.wav','sprites/carrot.webp'})
            (root/'src/effects.rs').write_text('')
            stage(root)
            self.assertFalse((root/'web/assets/pixelfx/fire/eruption.webp').exists())
if __name__=='__main__': unittest.main()
