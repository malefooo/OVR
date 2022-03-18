all: release

bins = ~/.cargo/ovr ~/.cargo/ovrd ~/.cargo/bin/tendermint /tmp/ovrd

lint:
	cargo clippy
	cargo check --tests
	cargo check --benches

test:
	cargo test --release -- --test-threads=1

testall: test
	cargo test --release \
		--no-default-features \
		--features="vsdb_rocksdb" \
		-- \
		--test-threads=1

bench:
	cargo bench

fmt:
	cargo +nightly fmt

fmtall:
	bash tools/fmt.sh

clean:
	cargo clean
	git stash
	git clean -fdx

update:
	cargo update

doc:
	cargo doc --open

define collect
	touch $(bins) && rm -rf $(bins)
	mkdir -p $(1) && rm -rf $(1)
	mkdir $(1)
	cp \
		./target/$(2)/$(1)/ovr \
		./target/$(2)/$(1)/ovrd \
		$(shell go env GOPATH)/bin/tendermint \
		$(1)/
	cd $(1) && ./ovrd pack
	rm $(1)/ovrd
	cp /tmp/ovrd $(1)/
	cp $(1)/* ~/.cargo/bin/
endef

build: tendermint
	cargo build --bins
	$(call collect,debug)

release: build_release

release_online: build_release_online

release_rocksdb: build_release_rocksdb

build_release: tendermint
	cargo build --release --bins
	$(call collect,release)

build_release_online: tendermint
	cargo build --release --bins --features="ruc_compact"
	$(call collect,release)

build_release_rocksdb: tendermint
	cargo build --release --bins --no-default-features --features="vsdb_rocksdb"
	$(call collect,release)

build_release_musl: tendermint
	cargo build --release --bins --target=x86_64-unknown-linux-musl
	$(call collect,release,x86_64-unknown-linux-musl)

tendermint: submod
	-@ rm $(shell which tendermint)
	cd tools/submodules/tendermint && $(MAKE) install

submod:
	git submodule update --init --recursive

prodenv:
	bash tools/create_prod_env.sh

run_prodenv:
	bash tools/create_prod_env.sh prodenv
	ovr dev -s -n prodenv

start_prodenv:
	ovr dev -s -n prodenv

stop_prodenv:
	ovr dev -S -n prodenv

destroy_prodenv:
	ovr dev -d -n prodenv

show_prodenv:
	ovr dev -i -n prodenv

######################################################################

# Mainnet_join_node:
# 	bash tools/testnet/add_fullnode.sh mainnet

Testnet_join_node:
	bash tools/testnet/add_fullnode.sh testnet

Testnet_start_node:
	bash tools/testnet/run_fullnode.sh

Testnet_stop_node:
	bash tools/testnet/kill_fullnode.sh

######################################################################

# CI 
ci_build_binary_rust_base:
	docker build -t binary-rust-base -f container/Dockerfile-binary-rust-base .

ci_build_dev_binary_image:
	sed -i "s/^ENV VERGEN_SHA_EXTERN .*/ENV VERGEN_SHA_EXTERN ${VERGEN_SHA_EXTERN}/g" container/Dockerfile-binary-image-release
	docker build -t ovrd-binary-image:$(IMAGE_TAG) -f container/Dockerfile-binary-image-dev .
	
ci_build_release_binary_image:
	sed -i "s/^ENV VERGEN_SHA_EXTERN .*/ENV VERGEN_SHA_EXTERN ${VERGEN_SHA_EXTERN}/g" container/Dockerfile-binary-image-release
	docker build -t ovrd-binary-image:$(IMAGE_TAG) -f container/Dockerfile-binary-image-release .

ci_build_image:
	@ if [ -d "./binary" ]; then \
		rm -rf ./binary || true; \
	fi
	@ docker run --rm -d --name ovrd-binary ovrd-binary-image:$(IMAGE_TAG)
	@ docker cp ovrd-binary:/binary ./binary
	@ docker rm -f ovrd-binary
	@ docker build -t $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG) -f container/Dockerfile-goleveldb .
ifeq ($(ENV),release)
	docker tag $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG) $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif

ci_push_image:
	docker push $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG)
ifeq ($(ENV),release)
	docker push $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif

clean_image:
	docker rmi $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG)
ifeq ($(ENV),release)
	docker rmi $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif

clean_binary_image:
	docker rmi ovrd-binary-image:$(IMAGE_TAG)
