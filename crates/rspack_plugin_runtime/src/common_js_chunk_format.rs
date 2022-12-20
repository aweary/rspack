use anyhow::anyhow;
use async_trait::async_trait;
use rspack_core::rspack_sources::{ConcatSource, RawSource, SourceExt};
use rspack_core::{
  runtime_globals, AdditionalChunkRuntimeRequirementsArgs, FilenameRenderOptions, Plugin,
  PluginAdditionalChunkRuntimeRequirementsOutput, PluginContext, PluginRenderChunkHookOutput,
  RenderChunkArgs,
};
use rspack_error::Result;
use rspack_plugin_javascript::runtime::{
  generate_chunk_entry_code, render_chunk_modules, render_chunk_runtime_modules,
};
#[derive(Debug)]
pub struct CommonJsChunkFormatPlugin {}

#[async_trait]
impl Plugin for CommonJsChunkFormatPlugin {
  fn name(&self) -> &'static str {
    "CommonJsChunkFormatPlugin"
  }

  fn apply(
    &mut self,
    _ctx: rspack_core::PluginContext<&mut rspack_core::ApplyContext>,
  ) -> Result<()> {
    Ok(())
  }

  fn additional_chunk_runtime_requirements(
    &self,
    _ctx: PluginContext,
    args: &mut AdditionalChunkRuntimeRequirementsArgs,
  ) -> PluginAdditionalChunkRuntimeRequirementsOutput {
    let compilation = &mut args.compilation;
    let chunk_ukey = args.chunk;
    let runtime_requirements = &mut args.runtime_requirements;
    let chunk = compilation
      .chunk_by_ukey
      .get(chunk_ukey)
      .ok_or_else(|| anyhow!("chunk not found"))?;

    if chunk.has_runtime(&compilation.chunk_group_by_ukey) {
      return Ok(());
    }

    if compilation
      .chunk_graph
      .get_number_of_entry_modules(chunk_ukey)
      > 0
    {
      runtime_requirements.insert(runtime_globals::REQUIRE.to_string());
      runtime_requirements.insert(runtime_globals::EXTERNAL_INSTALL_CHUNK.to_string());
    }

    Ok(())
  }

  fn render_chunk(
    &self,
    _ctx: PluginContext,
    args: &RenderChunkArgs,
  ) -> PluginRenderChunkHookOutput {
    let chunk = args.chunk();
    let mut sources = ConcatSource::default();
    sources.add(RawSource::from(format!(
      "exports.ids = ['{}'];\n",
      &chunk.id.to_owned()
    )));
    sources.add(RawSource::from("exports.modules = "));
    sources.add(render_chunk_modules(args.compilation, args.chunk_ukey)?);
    sources.add(RawSource::from(";\n"));
    if !args
      .compilation
      .chunk_graph
      .get_chunk_runtime_modules_in_order(args.chunk_ukey)
      .is_empty()
    {
      sources.add(RawSource::from("exports.runtime = "));
      sources.add(render_chunk_runtime_modules(
        args.compilation,
        args.chunk_ukey,
      )?);
      sources.add(RawSource::from(";\n"));
    }

    if chunk.has_entry_module(&args.compilation.chunk_graph) {
      let entry_point = {
        let entry_points = args
          .compilation
          .chunk_graph
          .get_chunk_entry_modules_with_chunk_group(&chunk.ukey);

        let entry_point_ukey = entry_points
          .iter()
          .next()
          .ok_or_else(|| anyhow!("should has entry point ukey"))?;

        args
          .compilation
          .chunk_group_by_ukey
          .get(entry_point_ukey)
          .ok_or_else(|| anyhow!("should has entry point"))?
      };

      let runtime_chunk_filename = {
        let runtime_chunk = args
          .compilation
          .chunk_by_ukey
          .get(&entry_point.get_runtime_chunk())
          .ok_or_else(|| anyhow!("should has runtime chunk"))?;

        let hash = Some(runtime_chunk.get_render_hash());
        args
          .compilation
          .options
          .output
          .chunk_filename
          .render(FilenameRenderOptions {
            filename: runtime_chunk.name.clone(),
            extension: Some(".js".to_string()),
            id: Some(runtime_chunk.id.clone()),
            contenthash: hash.clone(),
            chunkhash: hash.clone(),
            hash,
            ..Default::default()
          })
      };

      sources.add(RawSource::from(format!(
        "\nvar {} = require('./{}')",
        runtime_globals::REQUIRE,
        runtime_chunk_filename
      )));
      sources.add(RawSource::from(format!(
        "\n{}(exports)\n",
        runtime_globals::EXTERNAL_INSTALL_CHUNK,
      )));
      sources.add(generate_chunk_entry_code(args.compilation, args.chunk_ukey));
    }
    Ok(Some(sources.boxed()))
  }
}